//! UnrealScript bytecode interpreter.
//!
//! [`USToken`] enumerates every `EExprToken` member of
//! `HarryPotter2/Unreal/Core/Inc/UnStack.h` (no wildcard arms — adding an
//! opcode forces a compile error here). Decoding an unknown byte is a loud
//! [`Fail`] with reason `vm.unknown_token`; executing a known-but-deferred
//! token is a loud `vm.token_unsupported`, never silent progress.

use crate::arena::{BytecodeResolver, ObjectArena, ObjectData, ObjectId};
use crate::error::Fail;
use crate::name::Name;
use crate::natives::{CallArg, CallArgs, NativeRegistry};
use crate::props::PropStore;
use crate::value::PropValue;
#[cfg(test)]
use crate::value::PropertyKind;

/// Hard cap on nested script-function activations. Exceeding it is a loud
/// `vm.call_depth_exceeded` hard error (runaway recursion in shipped code
/// must never become a stack overflow).
pub const MAX_CALL_DEPTH: u32 = 32;

/// Upper bound on queued [`InstanceWrite`]s per activation.
pub const WRITE_QUEUE_BUDGET: usize = 4096;

/// Upper bound on queued [`LatentRequest`]s per activation (the scheduler
/// twin of [`WRITE_QUEUE_BUDGET`]).
pub const LATENT_REQUEST_BUDGET: usize = 4096;

/// Upper bound on engine-facing [`ScriptEffect`]s queued by one activation.
/// Effects are kept separate from property writes because an embedding engine
/// owns the world mutation boundary and drains them in script execution order.
pub const EFFECT_QUEUE_BUDGET: usize = 4096;

/// Engine-owned skeletal metadata lookup requested by a native. The VM
/// carries only the actor and authored name; mesh/package/PSA resolution stays
/// at the embedding boundary.
#[derive(Debug, Clone, PartialEq)]
pub enum SkeletalQuery {
    HasAnim { sequence: Name },
    BoneNumber { bone: Name },
    BoneName { index: i32 },
    IsAnimating { root_bone: Name },
    AnimGroup { sequence: Name },
}

/// A typed world mutation requested by a script native. Native bodies never
/// borrow an engine: they append one of these values to their [`Frame`], and
/// the embedding engine applies the ordered batch after the frame slice.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptEffect {
    /// Script `log(...)` text, retained by the embedding engine in execution
    /// order alongside its normal process log sink.
    Log {
        message: String,
    },
    SetLocation {
        actor: ObjectId,
        location: [f32; 3],
    },
    SetRotation {
        actor: ObjectId,
        rotation: [i32; 3],
    },
    SetPhysics {
        actor: ObjectId,
        physics: i32,
    },
    SetCollision {
        actor: ObjectId,
        colliding_actors: bool,
        block_actors: bool,
        block_players: bool,
    },
    SetCollisionSize {
        actor: ObjectId,
        radius: f32,
        height: f32,
        width: f32,
    },
    SetOwner {
        actor: ObjectId,
        owner: Option<ObjectId>,
    },
    SetBase {
        actor: ObjectId,
        base: Option<ObjectId>,
    },
    /// `AActor::MakeNoise` captured at the VM boundary. The engine selects
    /// loaded Pawn listeners and dispatches their `HearNoise` events.
    MakeNoise {
        actor: ObjectId,
        loudness: f32,
    },
    /// Prepend a pawn to its runtime level's `PawnList`. The engine owns
    /// resolution of `Actor.Level` and the two object-property writes.
    AddPawn {
        pawn: ObjectId,
    },
    SkeletalQuery {
        request_id: u64,
        actor: ObjectId,
        query: SkeletalQuery,
    },
    LocalizeRequest {
        request_id: u64,
        section: String,
        key: String,
        package: String,
    },
    CreateAnimChannelRequest {
        request_id: u64,
        reserved: ObjectId,
        actor: ObjectId,
        class: Option<ObjectId>,
        anim_type: i32,
        root_bone: Name,
        transient: bool,
        not_replaceable: bool,
    },
    SpawnRequest {
        request_id: u64,
        reserved: ObjectId,
        class: ObjectId,
        owner: Option<ObjectId>,
        location: [f32; 3],
        rotation: [i32; 3],
    },
    TraceActorsRequest {
        request_id: u64,
        source: ObjectId,
        base_class: Option<ObjectId>,
        start: [f32; 3],
        end: [f32; 3],
        extent: [f32; 3],
    },
    /// World-geometry-only line query used by `AActor::FastTrace`.
    /// The engine answers `true` precisely when the segment is clear.
    FastTraceRequest {
        request_id: u64,
        source: ObjectId,
        start: [f32; 3],
        end: [f32; 3],
    },
    /// Result-bearing `AActor::Trace` query. The frame retains the out
    /// lvalues while the engine resolves the collision.
    TraceRequest {
        request_id: u64,
        source: ObjectId,
        start: [f32; 3],
        end: [f32; 3],
        trace_actors: bool,
        extent: [f32; 3],
    },
    Destroy {
        actor: ObjectId,
    },
    PlaySound {
        actor: ObjectId,
        cue: ObjectId,
    },
    StopSound {
        actor: ObjectId,
        cue: Option<ObjectId>,
    },
    PlayMusic {
        actor: ObjectId,
        song: String,
        handle: i32,
    },
    StopMusic {
        actor: ObjectId,
        handle: i32,
    },
    StopAllMusic {
        actor: ObjectId,
    },
    AudioUnavailable {
        actor: Option<ObjectId>,
        source: String,
    },
    PlayAnim {
        actor: ObjectId,
        sequence: Name,
        rate: f32,
        looped: bool,
    },
    LoopAnim {
        actor: ObjectId,
        sequence: Name,
        rate: f32,
        looped: bool,
    },
    TweenAnim {
        actor: ObjectId,
        sequence: Name,
        rate: f32,
        looped: bool,
    },
    MoveSmooth {
        actor: ObjectId,
        delta: [f32; 3],
    },
    MoveTo {
        actor: ObjectId,
        destination: [f32; 3],
    },
    MoveToward {
        actor: ObjectId,
        target: ObjectId,
    },
    TurnTo {
        actor: ObjectId,
        focus: [f32; 3],
    },
    TurnToward {
        actor: ObjectId,
        target: ObjectId,
    },
    TriggerEvent {
        name: Name,
        other: Option<ObjectId>,
        instigator: Option<ObjectId>,
    },
    /// Engine-owned continuation of Pawn.CutCommand_FollowSpline. The
    /// authored function still performs capture/cue bookkeeping and writes
    /// its ordinary properties; this effect supplies the native
    /// PHYS_Interpolating movement which UnrealScript cannot implement.
    FollowSpline {
        actor: ObjectId,
        command: String,
        cue: String,
    },
}

/// Hard cap on executed opcodes per `Frame::run` — the execution-side twin
/// of the census walk's step budget; a runaway backward jump fails loudly
/// instead of hanging the tick loop.
const RUN_STEP_BUDGET: u64 = 1 << 22;

/// A parameter list owns its evaluated argument vector, so it gets an
/// independent cap and a strictly-forward cursor invariant. This catches
/// malformed nested expression jumps before they allocate unbounded args.
const PARM_PROGRESS_BUDGET: usize = 4096;

/// Seed of every frame's deterministic 32-bit LCG (`x = x * 1664525 +
/// 1013904223`, the Numerical-Recipes constants). `Rand`/`FRand`/`VRand`
/// draw from it so runs are reproducible; child frames inherit the
/// parent's live state and hand their final state back.
pub const FRAME_RNG_SEED: u32 = 0x2016_0611;

/// One deferred instance-variable write produced by `EX_Let` through an
/// `InstanceVariable`/`DefaultVariable` lvalue. The arena is borrowed
/// immutably by the frame, so writes queue here and the embedding caller
/// applies them with `ObjectArena::get_mut` once the run ends.
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceWrite {
    pub object: ObjectId,
    pub name: u32,
    pub value: PropValue,
}

/// Why a frame yielded mid-run: a latent function asked the embedding
/// scheduler to park it. [`Frame::run`] stops cleanly at the next opcode
/// boundary with the program cursor already PAST the suspending call, so
/// resuming continues after the latent — never restarting it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Suspend {
    /// `Sleep(seconds)` — wake after `seconds` of simulated time.
    Sleep(f64),
    /// `FinishAnim` — nominal placeholder wake (`hp-engine`'s
    /// `FINISH_ANIM_PLACEHOLDER_SECS`) until PSA animation playback lands.
    Anim,
    /// `FinishInterpolation` — poll the actor's `bInterpolating` flag on
    /// subsequent scheduler passes; no elapsed-time wake is implied.
    Interpolation,
    /// `GotoState` replaces the state frame immediately. The native queues
    /// its target then uses this signal to stop the abandoned activation at
    /// the call boundary; the embedding scheduler consumes the request
    /// instead of parking the old code.
    StateChange,
    /// Await the synchronous result of an engine-owned effect.
    EffectResult { request_id: u64 },
}

/// One non-suspending scheduler side effect a native body hands to its
/// embedder. The interpreter cannot reach the engine's timer wheel or the
/// actor's state machine, so `SetTimer`/`GotoState` queue here and the
/// embedding harness drains them with [`Frame::take_latent_requests`] once
/// the run yields.
#[derive(Debug, Clone, PartialEq)]
pub enum LatentRequest {
    /// `SetTimer(Seconds, bRepeating)` for the actor whose method issued it.
    SetTimer {
        actor: ObjectId,
        seconds: f32,
        repeating: bool,
    },
    /// Change the issuing actor's current script state.
    GotoState {
        actor: ObjectId,
        change: StateChange,
    },
    /// Enable or disable one probe event on the issuing actor's active state.
    SetEventEnabled {
        actor: ObjectId,
        event: String,
        enabled: bool,
    },
}

/// The first argument of `GotoState`, preserving the three distinct UE1
/// meanings that a value-only `Option<String>` would collapse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateChange {
    /// The optional argument was omitted: keep the current state.
    Current,
    /// `NAME_None`: leave state execution and return to the class frame.
    None,
    /// `NAME_Auto`: select the first `STATE_Auto` state on the class chain.
    Auto,
    /// Enter a named state.
    Named(String),
}

/// Serializable nested script activation retained when a callee yields.
/// The outer frame already sits past the call site; on wake it restores this
/// record first, then resumes its own code only after the callee completes.
#[derive(Debug, Clone)]
pub struct SuspendedCall {
    code: Vec<u8>,
    pc: usize,
    /// Contextual dispatch target captured at the call boundary.
    self_id: Option<ObjectId>,
    /// Lexical `Self` captured independently of a temporary Context target.
    lexical_self_id: Option<ObjectId>,
    locals: PropStore,
    result: PropValue,
    call_depth: u32,
    rng_state: u32,
    active_state: Option<u32>,
    /// Resolver belonging to the suspended callee's own bytecode stream.
    resolver: BytecodeResolver,
    /// State scope used for virtual lookup while this activation resumes.
    active_state_object: Option<ObjectId>,
    active_state_owner: Option<ObjectId>,
    nested: Option<Box<SuspendedCall>>,
    iterators: Vec<IteratorCursor>,
    next_effect_request_id: u64,
    reserved_object_count: u32,
    pending_expression: Option<PendingExpression>,
    pending_effect_request: Option<u64>,
    suspended_call_arg: Option<(Option<u32>, Option<LValue>)>,
    pending_follow_spline: Option<ScriptEffect>,
}

/// A stable, typed script lvalue captured for a native `out` parameter.
/// Recursive variants model UE1's address expressions without exposing raw
/// pointers: writes rebuild the containing array/struct and then propagate to
/// the root local or object property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LValue {
    Local(u32),
    Instance {
        actor: ObjectId,
        name: u32,
    },
    Default {
        object: ObjectId,
        name: u32,
    },
    ArrayElement {
        base: Box<LValue>,
        index: usize,
    },
    StructField {
        base: Box<LValue>,
        field: u32,
        property: ObjectId,
    },
}

type LvalueTarget = LValue;

/// VM work immediately outside a result-bearing native expression. It is
/// serialized with the activation so the engine can inject the real result
/// before the enclosing bytecode operation observes it.
#[derive(Debug, Clone)]
pub enum PendingExpression {
    Noop,
    Assign {
        target: LValue,
        bool_coerce: bool,
    },
    Return,
    Context {
        class_context: bool,
    },
    JumpIfNot {
        runtime_target: usize,
    },
    Convert {
        token: USToken,
    },
    Cast {
        token: USToken,
        class: ObjectId,
    },
    NativeCall {
        slot: u16,
        params: Vec<ObjectId>,
        args: CallArgs,
        pending_name: Option<u32>,
        pending_lvalue: Option<LValue>,
    },
    ResolvedCall {
        callee: ResolvedCallee,
        args: CallArgs,
        pending_name: Option<u32>,
        pending_lvalue: Option<LValue>,
    },
    BeginTraceIterator {
        actor: LValue,
        hit_location: LValue,
        hit_normal: LValue,
    },
    BindTrace {
        hit_location: LValue,
        hit_normal: LValue,
    },
    Sequence {
        inner: Box<PendingExpression>,
        outer: Box<PendingExpression>,
    },
}

/// One item in a resumable native iterator. TraceActors carries two
/// additional out-parameter values beside the actor.
#[derive(Debug, Clone)]
struct IteratorItem {
    actor: ObjectId,
    hit_location: Option<PropValue>,
    hit_normal: Option<PropValue>,
}

/// A resumable iterator loop. Candidate ids are retained in deterministic
/// engine order; `index` points at the item currently bound to its outputs.
#[derive(Debug, Clone)]
pub struct IteratorCursor {
    values: Vec<IteratorItem>,
    index: usize,
    output: LvalueTarget,
    hit_location: Option<LvalueTarget>,
    hit_normal: Option<LvalueTarget>,
    clear_actor_on_exhaust: bool,
    body_pc: usize,
    end_pc: usize,
}

#[derive(Debug, Clone)]
struct PendingIterator {
    values: Vec<IteratorItem>,
    output: LvalueTarget,
    hit_location: Option<LvalueTarget>,
    hit_normal: Option<LvalueTarget>,
    clear_actor_on_exhaust: bool,
}

#[derive(Debug, Clone)]
pub struct ResolvedCallee {
    function_id: ObjectId,
    /// Parameter property objects in declaration order; names and kinds
    /// drive argument binding and omitted-arg defaults.
    params: Vec<ObjectId>,
    native_index: u16,
}

pub type VmResult<T> = std::result::Result<T, Fail>;

/// Every `EExprToken` from `UnStack.h`, tagged with its opcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum USToken {
    LocalVariable = 0x00,
    InstanceVariable = 0x01,
    DefaultVariable = 0x02,
    Return = 0x04,
    Switch = 0x05,
    Jump = 0x06,
    JumpIfNot = 0x07,
    Stop = 0x08,
    Assert = 0x09,
    Case = 0x0A,
    Nothing = 0x0B,
    LabelTable = 0x0C,
    GotoLabel = 0x0D,
    EatString = 0x0E,
    Let = 0x0F,
    DynArrayElement = 0x10,
    New = 0x11,
    ClassContext = 0x12,
    MetaCast = 0x13,
    LetBool = 0x14,
    LineNumber = 0x15,
    EndFunctionParms = 0x16,
    SelfToken = 0x17,
    Skip = 0x18,
    Context = 0x19,
    ArrayElement = 0x1A,
    VirtualFunction = 0x1B,
    FinalFunction = 0x1C,
    IntConst = 0x1D,
    FloatConst = 0x1E,
    StringConst = 0x1F,
    ObjectConst = 0x20,
    NameConst = 0x21,
    RotationConst = 0x22,
    VectorConst = 0x23,
    ByteConst = 0x24,
    IntZero = 0x25,
    IntOne = 0x26,
    True = 0x27,
    False = 0x28,
    NativeParm = 0x29,
    NoObject = 0x2A,
    IntConstByte = 0x2C,
    BoolVariable = 0x2D,
    DynamicCast = 0x2E,
    Iterator = 0x2F,
    IteratorPop = 0x30,
    IteratorNext = 0x31,
    StructCmpEq = 0x32,
    StructCmpNe = 0x33,
    UnicodeStringConst = 0x34,
    StructMember = 0x36,
    DynArrayCount = 0x37,
    DebugInfo = 0x38,
    GlobalFunction = 0x39,
    RotatorToVector = 0x3A,
    ByteToInt = 0x3B,
    ByteToBool = 0x3C,
    ByteToFloat = 0x3D,
    IntToByte = 0x3E,
    IntToBool = 0x3F,
    IntToFloat = 0x40,
    BoolToByte = 0x41,
    BoolToInt = 0x42,
    BoolToFloat = 0x43,
    FloatToByte = 0x44,
    FloatToInt = 0x45,
    FloatToBool = 0x46,
    ObjectToBool = 0x47,
    NameToBool = 0x48,
    StringToByte = 0x49,
    StringToInt = 0x4A,
    StringToBool = 0x4B,
    StringToFloat = 0x4C,
    StringToVector = 0x4D,
    StringToRotator = 0x4E,
    VectorToBool = 0x4F,
    VectorToRotator = 0x50,
    RotatorToBool = 0x51,
    ByteToString = 0x52,
    IntToString = 0x53,
    BoolToString = 0x54,
    FloatToString = 0x55,
    ObjectToString = 0x56,
    NameToString = 0x57,
    VectorToString = 0x58,
    RotatorToString = 0x59,
    StringToName = 0x5A,
    ExtendedNative = 0x60,
    FirstNative = 0x70,
}

impl USToken {
    /// Decode one opcode. Unknown bytes are loud errors at the call site.
    pub fn from_opcode(opcode: u8) -> Option<USToken> {
        Some(match opcode {
            0x00 => USToken::LocalVariable,
            0x01 => USToken::InstanceVariable,
            0x02 => USToken::DefaultVariable,
            0x04 => USToken::Return,
            0x05 => USToken::Switch,
            0x06 => USToken::Jump,
            0x07 => USToken::JumpIfNot,
            0x08 => USToken::Stop,
            0x09 => USToken::Assert,
            0x0A => USToken::Case,
            0x0B => USToken::Nothing,
            0x0C => USToken::LabelTable,
            0x0D => USToken::GotoLabel,
            0x0E => USToken::EatString,
            0x0F => USToken::Let,
            0x10 => USToken::DynArrayElement,
            0x11 => USToken::New,
            0x12 => USToken::ClassContext,
            0x13 => USToken::MetaCast,
            0x14 => USToken::LetBool,
            0x15 => USToken::LineNumber,
            0x16 => USToken::EndFunctionParms,
            0x17 => USToken::SelfToken,
            0x18 => USToken::Skip,
            0x19 => USToken::Context,
            0x1A => USToken::ArrayElement,
            0x1B => USToken::VirtualFunction,
            0x1C => USToken::FinalFunction,
            0x1D => USToken::IntConst,
            0x1E => USToken::FloatConst,
            0x1F => USToken::StringConst,
            0x20 => USToken::ObjectConst,
            0x21 => USToken::NameConst,
            0x22 => USToken::RotationConst,
            0x23 => USToken::VectorConst,
            0x24 => USToken::ByteConst,
            0x25 => USToken::IntZero,
            0x26 => USToken::IntOne,
            0x27 => USToken::True,
            0x28 => USToken::False,
            0x29 => USToken::NativeParm,
            0x2A => USToken::NoObject,
            0x2C => USToken::IntConstByte,
            0x2D => USToken::BoolVariable,
            0x2E => USToken::DynamicCast,
            0x2F => USToken::Iterator,
            0x30 => USToken::IteratorPop,
            0x31 => USToken::IteratorNext,
            0x32 => USToken::StructCmpEq,
            0x33 => USToken::StructCmpNe,
            0x34 => USToken::UnicodeStringConst,
            0x36 => USToken::StructMember,
            0x37 => USToken::DynArrayCount,
            0x38 => USToken::DebugInfo,
            0x39 => USToken::GlobalFunction,
            0x3A => USToken::RotatorToVector,
            0x3B => USToken::ByteToInt,
            0x3C => USToken::ByteToBool,
            0x3D => USToken::ByteToFloat,
            0x3E => USToken::IntToByte,
            0x3F => USToken::IntToBool,
            0x40 => USToken::IntToFloat,
            0x41 => USToken::BoolToByte,
            0x42 => USToken::BoolToInt,
            0x43 => USToken::BoolToFloat,
            0x44 => USToken::FloatToByte,
            0x45 => USToken::FloatToInt,
            0x46 => USToken::FloatToBool,
            0x47 => USToken::ObjectToBool,
            0x48 => USToken::NameToBool,
            0x49 => USToken::StringToByte,
            0x4A => USToken::StringToInt,
            0x4B => USToken::StringToBool,
            0x4C => USToken::StringToFloat,
            0x4D => USToken::StringToVector,
            0x4E => USToken::StringToRotator,
            0x4F => USToken::VectorToBool,
            0x50 => USToken::VectorToRotator,
            0x51 => USToken::RotatorToBool,
            0x52 => USToken::ByteToString,
            0x53 => USToken::IntToString,
            0x54 => USToken::BoolToString,
            0x55 => USToken::FloatToString,
            0x56 => USToken::ObjectToString,
            0x57 => USToken::NameToString,
            0x58 => USToken::VectorToString,
            0x59 => USToken::RotatorToString,
            0x5A => USToken::StringToName,
            0x60 => USToken::ExtendedNative,
            0x70 => USToken::FirstNative,
            _ => return None,
        })
    }
}

/// One activation record: locals, result propagation, and the code cursor.
/// Natives receive `&mut Frame` (locals, self, deterministic RNG) plus the
/// call's evaluated [`CallArgs`].
pub struct Frame<'a> {
    pub arena: &'a ObjectArena,
    /// Locals and parameters of this activation, keyed by pool name index.
    pub locals: PropStore,
    /// The object executing the code (`self`). Temporarily re-targeted
    /// while a `Context`/`ClassContext` member expression evaluates.
    pub self_id: Option<ObjectId>,
    /// Lexical `Self` for this activation. Member contexts retarget property
    /// and function lookup, but their argument expressions still see the
    /// caller's self (`Level.Game.IsRelevant(Self)` passes the actor).
    lexical_self_id: Option<ObjectId>,
    /// Resolver for compact package object operands in this frame's code.
    resolver: BytecodeResolver,
    /// Pending result of the last evaluated expression.
    pub result: PropValue,
    registry: &'a NativeRegistry,
    code: Vec<u8>,
    runtime_offsets: std::collections::BTreeMap<usize, usize>,
    offset_map_abort: Option<CensusAbort>,
    pc: usize,
    /// Script-call nesting depth of this activation (`Frame::new` = 0).
    call_depth: u32,
    /// Deferred instance/default-variable writes; drained by the caller.
    pending_writes: Vec<InstanceWrite>,
    /// Writes inherited from an enclosing activation. They participate in
    /// read-through overlay lookup but are not re-emitted when this frame is
    /// drained.
    inherited_writes: Vec<InstanceWrite>,
    /// Engine-facing mutations emitted by native bodies. The engine drains
    /// these after a frame slice, preserving bytecode execution order.
    effects: Vec<ScriptEffect>,
    /// Camera spline effect retained across nested suspension until the
    /// authored CutCommand_FollowSpline returns true.
    pending_follow_spline: Option<ScriptEffect>,
    /// Live LCG state for `Rand`/`FRand`/`VRand`.
    rng_state: u32,
    /// Executed-opcode counter against `RUN_STEP_BUDGET`.
    steps: u64,
    /// Set by a latent body (`Sleep`/`FinishAnim`); [`Frame::run`] stops
    /// at the next loop check. First suspension wins.
    suspend: Option<Suspend>,
    /// Scheduler side effects queued by native bodies (drained by the
    /// embedder after the run).
    latent_requests: Vec<LatentRequest>,
    /// Active `foreach` cursor stack. Nested iterator expressions nest here
    /// exactly as Unreal's PRE_ITERATOR/POST_ITERATOR stack does.
    iterators: Vec<IteratorCursor>,
    /// Candidate set produced by the native expression immediately inside an
    /// `EX_Iterator`; activated only after its end offset is decoded.
    pending_iterator: Option<PendingIterator>,
    /// Out bindings captured by a result-bearing iterator native until the
    /// surrounding EX_Iterator records its resumable continuation.
    pending_trace_iterator: Option<(LValue, LValue, LValue)>,
    /// Deterministic active-actor snapshot supplied by the embedding engine
    /// for native iterators. `None` keeps standalone VM tests arena-scoped.
    actor_scope: Option<Vec<ObjectId>>,
    /// Name-pool index of the state whose code this frame executes, set
    /// by the embedder (`GetStateName`/`IsInState` read it). `None` in
    /// plain function context.
    active_state: Option<u32>,
    /// The concrete state object that owns `active_state`, when execution is
    /// in a state frame. Virtual calls search it before the class chain.
    active_state_object: Option<ObjectId>,
    active_state_owner: Option<ObjectId>,
    /// A yielded script callee that must complete before this activation can
    /// execute the instruction after its call site.
    suspended_call: Option<Box<SuspendedCall>>,
    /// Slot currently executing through the native registry. Deferred bodies
    /// use it to report the exact ABI entry and registered subject.
    current_native_slot: Option<u16>,
    /// Monotonic engine-owned id source for result-bearing effects.
    next_effect_request_id: u64,
    /// Arena slots reserved by result-bearing effects in this slice.
    reserved_object_count: u32,
    pending_expression: Option<PendingExpression>,
    pending_effect_request: Option<u64>,
    suspended_call_arg: Option<(Option<u32>, Option<LValue>)>,
}

impl<'a> Frame<'a> {
    pub fn new(
        arena: &'a ObjectArena,
        registry: &'a NativeRegistry,
        code: &[u8],
        self_id: Option<ObjectId>,
    ) -> Self {
        Self::with_resolver(arena, registry, code, self_id, BytecodeResolver::default())
    }

    /// Create a frame for a loaded function or state. The resolver is owned
    /// by that code stream's package and survives nested/latent continuations.
    pub fn with_resolver(
        arena: &'a ObjectArena,
        registry: &'a NativeRegistry,
        code: &[u8],
        self_id: Option<ObjectId>,
        resolver: BytecodeResolver,
    ) -> Self {
        let (runtime_offsets, offset_map_abort) = build_runtime_offset_map(arena, &resolver, code);
        Self {
            arena,
            locals: PropStore::new(),
            self_id,
            lexical_self_id: self_id,
            result: PropValue::Int(0),
            registry,
            resolver,
            runtime_offsets,
            offset_map_abort,
            code: code.to_vec(),
            pc: 0,
            call_depth: 0,
            pending_writes: Vec::new(),
            inherited_writes: Vec::new(),
            effects: Vec::new(),
            pending_follow_spline: None,
            rng_state: FRAME_RNG_SEED,
            steps: 0,
            suspend: None,
            latent_requests: Vec::new(),
            iterators: Vec::new(),
            pending_iterator: None,
            pending_trace_iterator: None,
            actor_scope: None,
            active_state: None,
            active_state_object: None,
            active_state_owner: None,
            suspended_call: None,
            current_native_slot: None,
            next_effect_request_id: 0,
            reserved_object_count: 0,
            pending_expression: None,
            pending_effect_request: None,
            suspended_call_arg: None,
        }
    }

    fn defer_expression(&mut self, outer: PendingExpression) {
        self.pending_expression = Some(match self.pending_expression.take() {
            Some(inner) => PendingExpression::Sequence {
                inner: Box::new(inner),

                outer: Box::new(outer),
            },
            None => outer,
        });
    }
    fn awaiting_effect_result(&self) -> bool {
        matches!(self.suspend, Some(Suspend::EffectResult { .. }))
    }

    fn defer_native_call(&mut self, slot: u16, params: Vec<ObjectId>, args: CallArgs) {
        let (pending_name, pending_lvalue) = self
            .suspended_call_arg
            .take()
            .expect("suspended native argument");
        self.defer_expression(PendingExpression::NativeCall {
            slot,
            params,
            args,
            pending_name,
            pending_lvalue,
        });
    }

    fn defer_resolved_call(&mut self, callee: ResolvedCallee, args: CallArgs) {
        let (pending_name, pending_lvalue) = self
            .suspended_call_arg
            .take()
            .expect("suspended resolved-call argument");
        self.defer_expression(PendingExpression::ResolvedCall {
            callee,
            args,
            pending_name,
            pending_lvalue,
        });
    }

    fn resume_pending_expression(&mut self) -> VmResult<()> {
        let Some(pending) = self.pending_expression.take() else {
            return Ok(());
        };
        self.resume_pending_node(pending)
    }

    fn resume_pending_node(&mut self, pending: PendingExpression) -> VmResult<()> {
        match pending {
            PendingExpression::Noop => {}
            PendingExpression::JumpIfNot { runtime_target } => {
                if !self.result.truthy() {
                    self.pc = self.serialized_offset(runtime_target)?;
                }
            }
            PendingExpression::Convert { token } => {
                self.result = convert(self.arena, token, &self.result)?;
            }
            PendingExpression::Cast { token, class } => {
                self.result = cast(self.arena, token, class, &self.result)?;
            }
            PendingExpression::Assign {
                target,
                bool_coerce,
            } => {
                let mut value = self.result.clone();
                if bool_coerce && !matches!(value, PropValue::Bool(_)) {
                    value = PropValue::Bool(value.truthy());
                }
                self.assign_lvalue(target, value)?;
            }
            PendingExpression::BindTrace {
                hit_location,
                hit_normal,
            } => {
                let PropValue::FixedArray(mut fields) =
                    std::mem::replace(&mut self.result, PropValue::Int(0))
                else {
                    return Err(Fail::new(
                        "native.trace_collision_result_invalid",
                        "Trace engine result is not an actor/location/normal tuple",
                    ));
                };
                if fields.len() != 3 {
                    return Err(Fail::new(
                        "native.trace_collision_result_invalid",
                        format!("Trace result has {} fields, expected 3", fields.len()),
                    ));
                }
                let normal = fields.pop().expect("length checked");
                let location = fields.pop().expect("length checked");
                let actor = fields.pop().expect("length checked");
                let valid_actor = matches!(actor, PropValue::Object(None))
                    || matches!(actor, PropValue::Object(Some(raw)) if raw >= 0);
                if !valid_actor {
                    return Err(Fail::new(
                        "native.trace_collision_result_invalid",
                        format!("Trace result actor is invalid: {actor:?}"),
                    ));
                }
                self.assign_lvalue(hit_location, location)?;
                self.assign_lvalue(hit_normal, normal)?;
                // The engine tuple is now fully consumed. Allow the enclosing
                // expression to receive the actor rather than stopping on the
                // marker that suspended this native before its placeholder
                // return was installed.
                self.suspend = None;
                self.result = actor;
            }
            PendingExpression::BeginTraceIterator {
                actor,
                hit_location,
                hit_normal,
            } => {
                let PropValue::Array(hits) = std::mem::replace(&mut self.result, PropValue::Int(0))
                else {
                    return Err(Fail::new(
                        "native.trace_collision_result_invalid",
                        "TraceActors engine result is not an array",
                    ));
                };
                let mut values = Vec::with_capacity(hits.len());
                for hit in hits {
                    let PropValue::FixedArray(mut fields) = hit else {
                        return Err(Fail::new(
                            "native.trace_collision_result_invalid",
                            "TraceActors hit is not a fixed actor/location/normal tuple",
                        ));
                    };
                    if fields.len() != 3 {
                        return Err(Fail::new(
                            "native.trace_collision_result_invalid",
                            format!("TraceActors hit has {} fields, expected 3", fields.len()),
                        ));
                    }
                    let normal = fields.pop().expect("length checked");
                    let location = fields.pop().expect("length checked");
                    let actor_id = match fields.pop().expect("length checked") {
                        PropValue::Object(Some(raw)) if raw >= 0 => ObjectId(raw as u32),
                        other => {
                            return Err(Fail::new(
                                "native.trace_collision_result_invalid",
                                format!("TraceActors hit actor is invalid: {other:?}"),
                            ));
                        }
                    };
                    values.push(IteratorItem {
                        actor: actor_id,
                        hit_location: Some(location),
                        hit_normal: Some(normal),
                    });
                }
                self.pending_iterator = Some(PendingIterator {
                    values,
                    output: actor,
                    hit_location: Some(hit_location),
                    hit_normal: Some(hit_normal),
                    clear_actor_on_exhaust: true,
                });
                self.activate_iterator_cursor()?;
            }
            PendingExpression::Return => self.pc = self.code.len(),
            PendingExpression::Context { class_context } => {
                let ctx = std::mem::replace(&mut self.result, PropValue::Int(0));
                let skip = usize::from(self.u16_at()?);
                let _zero_fill = self.u8_at()?;
                let target = match ctx {
                    PropValue::Object(Some(raw)) if raw >= 0 => ObjectId(raw as u32),
                    _ => {
                        self.result = self.context_member_zero()?;
                        self.pc = self.serialized_relative_target(self.pc, skip)?;
                        return Ok(());
                    }
                };
                let member_self = if class_context {
                    match &self.arena.get(target)?.data {
                        ObjectData::Class(data) => data.default_object.ok_or_else(|| {
                            Fail::new(
                                "vm.token_unsupported",
                                "ClassContext on a class without a default object is deferred",
                            )
                        })?,
                        _ => {
                            return Err(Fail::new(
                                "vm.object_ref_type",
                                format!("EX_ClassContext target {target:?} is not a UClass"),
                            ));
                        }
                    }
                } else {
                    target
                };
                let saved = self.self_id.replace(member_self);
                let member = self.expr();
                self.self_id = saved;
                member?;
            }
            PendingExpression::NativeCall {
                slot,
                params,
                mut args,
                pending_name,
                pending_lvalue,
            } => {
                args.push(CallArg {
                    name: pending_name,
                    value: self.result.clone(),
                    lvalue: pending_lvalue,
                });
                let remaining = params.get(args.len()..).unwrap_or(&[]);
                let more = self.collect_parms(remaining, Some(slot))?;
                for arg in more.iter().cloned() {
                    args.push(arg);
                }
                if self.awaiting_effect_result() {
                    let (pending_name, pending_lvalue) = self
                        .suspended_call_arg
                        .take()
                        .expect("suspended call argument");
                    self.defer_expression(PendingExpression::NativeCall {
                        slot,
                        params,
                        args,
                        pending_name,
                        pending_lvalue,
                    });
                    return Ok(());
                }
                self.result = self.invoke_native(slot, &args)?;
            }
            PendingExpression::ResolvedCall {
                callee,
                mut args,
                pending_name,
                pending_lvalue,
            } => {
                args.push(CallArg {
                    name: pending_name,
                    value: self.result.clone(),
                    lvalue: pending_lvalue,
                });
                let remaining = callee.params.get(args.len()..).unwrap_or(&[]);
                let more = self.collect_parms(
                    remaining,
                    (callee.native_index != 0).then_some(callee.native_index),
                )?;
                for arg in more.iter().cloned() {
                    args.push(arg);
                }
                if self.awaiting_effect_result() {
                    let (pending_name, pending_lvalue) = self
                        .suspended_call_arg
                        .take()
                        .expect("suspended call argument");
                    self.defer_expression(PendingExpression::ResolvedCall {
                        callee,
                        args,
                        pending_name,
                        pending_lvalue,
                    });
                    return Ok(());
                }
                self.dispatch_resolved(&callee, &args)?;
            }
            PendingExpression::Sequence { inner, outer } => {
                self.resume_pending_node(*inner)?;
                if self.suspend.is_some() {
                    let inner = self
                        .pending_expression
                        .take()
                        .unwrap_or(PendingExpression::Noop);
                    self.pending_expression = Some(PendingExpression::Sequence {
                        inner: Box::new(inner),
                        outer,
                    });
                    return Ok(());
                }
                self.resume_pending_node(*outer)?;
            }
        }
        Ok(())
    }
    /// Run to `EX_Return`, end-of-code, or the first latent suspension.
    /// A suspension stops at an opcode boundary with `pc` already past the
    /// suspending call. If that call was a script callee, its serialized
    /// activation is resumed to completion before this frame continues.
    pub fn run(&mut self) -> VmResult<PropValue> {
        if self.suspended_call.is_some() {
            self.resume_suspended_call()?;
            if self.suspend.is_some() {
                return Ok(self.result.clone());
            }
        }
        self.resume_pending_expression()?;
        if self.suspend.is_some() || self.pc >= self.code.len() {
            return Ok(self.result.clone());
        }
        while self.pc < self.code.len() && self.suspend.is_none() {
            self.steps += 1;
            if self.steps > RUN_STEP_BUDGET {
                return Err(Fail::new(
                    "vm.step_budget_exceeded",
                    format!("run exceeded {RUN_STEP_BUDGET} opcodes (backward jump loop?)"),
                ));
            }
            let opcode = self.code[self.pc];
            self.pc += 1;
            if (0x61..=0x6F).contains(&opcode) {
                // native slot is `(opcode - 0x60)*0x100 + B` (SerializeExpr).
                let low = self.u8_at()?;
                let slot = u16::from(opcode - 0x60) * 0x100 + u16::from(low);
                let args = self.collect_parms(&[], Some(slot))?;
                if self.awaiting_effect_result() {
                    self.defer_native_call(slot, Vec::new(), args);
                    continue;
                }
                self.result = self.invoke_native(slot, &args)?;
                continue;
            }
            if (0x70..=0xFF).contains(&opcode) {
                // Opcodes at or above EX_FirstNative ARE native slots:
                // GNatives[opcode] with a normal parameter list.
                self.dispatch_native(u16::from(opcode))?;
                continue;
            }
            let Some(token) = USToken::from_opcode(opcode) else {
                return Err(unknown_token(opcode, self.pc - 1));
            };
            if self.exec(token)? {
                break; // EX_Return executed.
            }
        }
        Ok(self.result.clone())
    }

    fn resume_suspended_call(&mut self) -> VmResult<()> {
        let snapshot = *self
            .suspended_call
            .take()
            .expect("checked suspended nested call");
        let mut visible_writes = self.inherited_writes.clone();
        visible_writes.extend(self.pending_writes.iter().cloned());
        let mut nested = Self::from_suspended_call(
            self.arena,
            self.registry,
            snapshot,
            self.actor_scope.clone(),
            visible_writes,
        );
        let outcome = nested.run();
        if nested.suspend.is_none() && outcome.is_ok() {
            nested.finalize_pending_follow_spline();
        }
        self.absorb_nested_effects(&mut nested);
        if nested.suspend.is_some() {
            self.suspended_call = Some(Box::new(nested.snapshot_suspended_call()));
            if self.suspend.is_none() {
                self.suspend = nested.take_suspend();
            }
            outcome?;
            return Ok(());
        }
        outcome?;
        self.result = nested.result;
        Ok(())
    }

    fn finalize_pending_follow_spline(&mut self) {
        let Some(effect) = self.pending_follow_spline.take() else {
            return;
        };
        if self.result == PropValue::Bool(true) {
            self.effects.push(effect);
        }
    }
    fn absorb_nested_effects(&mut self, nested: &mut Frame<'a>) {
        const LOG_EFFECT_BUDGET: usize = 256;
        self.pending_writes.extend(nested.pending_writes.drain(..));
        let mut logs = self
            .effects
            .iter()
            .filter(|effect| matches!(effect, ScriptEffect::Log { .. }))
            .count();
        for effect in nested.effects.drain(..) {
            if matches!(effect, ScriptEffect::Log { .. }) {
                if logs >= LOG_EFFECT_BUDGET {
                    continue;
                }
                logs += 1;
            }
            self.effects.push(effect);
        }
        self.rng_state = nested.rng_state;
        self.latent_requests
            .extend(std::mem::take(&mut nested.latent_requests));
        self.next_effect_request_id = nested.next_effect_request_id;
        self.reserved_object_count = nested.reserved_object_count;
    }

    fn snapshot_suspended_call(&self) -> SuspendedCall {
        SuspendedCall {
            code: self.code.clone(),
            pc: self.pc,
            self_id: self.self_id,
            lexical_self_id: self.lexical_self_id,
            locals: self.locals.clone(),
            result: self.result.clone(),
            call_depth: self.call_depth,
            rng_state: self.rng_state,
            active_state: self.active_state,
            resolver: self.resolver.clone(),
            active_state_object: self.active_state_object,
            active_state_owner: self.active_state_owner,
            nested: self.suspended_call.clone(),
            iterators: self.iterators.clone(),
            next_effect_request_id: self.next_effect_request_id,
            reserved_object_count: self.reserved_object_count,
            pending_expression: self.pending_expression.clone(),
            pending_effect_request: self.pending_effect_request,
            suspended_call_arg: self.suspended_call_arg.clone(),
            pending_follow_spline: self.pending_follow_spline.clone(),
        }
    }

    fn from_suspended_call(
        arena: &'a ObjectArena,
        registry: &'a NativeRegistry,
        snapshot: SuspendedCall,
        actor_scope: Option<Vec<ObjectId>>,
        inherited_writes: Vec<InstanceWrite>,
    ) -> Self {
        let (runtime_offsets, offset_map_abort) =
            build_runtime_offset_map(arena, &snapshot.resolver, &snapshot.code);
        Self {
            arena,
            locals: snapshot.locals,
            self_id: snapshot.self_id,
            lexical_self_id: snapshot.lexical_self_id,
            result: snapshot.result,
            registry,
            resolver: snapshot.resolver,
            runtime_offsets,
            offset_map_abort,
            code: snapshot.code,
            pc: snapshot.pc,
            call_depth: snapshot.call_depth,
            pending_writes: Vec::new(),
            inherited_writes,
            effects: Vec::new(),
            pending_follow_spline: snapshot.pending_follow_spline,
            rng_state: snapshot.rng_state,
            steps: 0,
            suspend: None,
            latent_requests: Vec::new(),
            iterators: snapshot.iterators,
            pending_iterator: None,
            pending_trace_iterator: None,
            actor_scope,
            active_state: snapshot.active_state,
            active_state_object: snapshot.active_state_object,
            active_state_owner: snapshot.active_state_owner,
            suspended_call: snapshot.nested,
            current_native_slot: None,
            next_effect_request_id: snapshot.next_effect_request_id,
            reserved_object_count: snapshot.reserved_object_count,
            pending_expression: snapshot.pending_expression,
            pending_effect_request: snapshot.pending_effect_request,
            suspended_call_arg: snapshot.suspended_call_arg,
        }
    }
    fn serialized_offset(&self, runtime_offset: usize) -> VmResult<usize> {
        offset_from_map(
            &self.runtime_offsets,
            self.offset_map_abort.as_ref(),
            runtime_offset,
        )
    }

    fn serialized_relative_target(
        &self,
        serialized_base: usize,
        runtime_delta: usize,
    ) -> VmResult<usize> {
        relative_target_from_map(
            &self.runtime_offsets,
            self.offset_map_abort.as_ref(),
            serialized_base,
            runtime_delta,
        )
    }
    /// Resolve a package-local object value retained in authored defaults.
    ///
    /// Bytecode operands are resolved while decoding the instruction, but
    /// class/object properties can still carry their signed linker reference
    /// into a native argument. Only negative import references take this
    /// path; non-negative `PropValue::Object` values are already arena ids.
    pub(crate) fn resolve_imported_object(&self, raw: i32) -> VmResult<Option<ObjectId>> {
        debug_assert!(raw < 0);
        self.resolver.resolve(raw).map_err(|fail| {
            let subject = self
                .self_id
                .and_then(|id| self.arena.path_of(id).ok())
                .unwrap_or_else(|| "None".to_string());
            Fail::new(
                fail.reason_code,
                format!(
                    "{} (executing package {}, self {subject})",
                    fail.message,
                    self.resolver.package()
                ),
            )
        })
    }

    fn u16_at(&mut self) -> VmResult<u16> {
        let end = self.pc + 2;
        let v = u16::from_le_bytes(
            self.code
                .get(self.pc..end)
                .ok_or_else(truncated)?
                .try_into()
                .unwrap(),
        );
        self.pc = end;
        Ok(v)
    }

    fn i32_at(&mut self) -> VmResult<i32> {
        let end = self.pc + 4;
        let v = i32::from_le_bytes(
            self.code
                .get(self.pc..end)
                .ok_or_else(truncated)?
                .try_into()
                .unwrap(),
        );
        self.pc = end;
        Ok(v)
    }

    fn name_ci(&mut self) -> VmResult<i32> {
        let cursor = &mut self.pc;
        let _ = cursor;
        // Compact index straight from the code bytes.
        let mut pos = self.pc;
        let first = *self.code.get(pos).ok_or_else(truncated)? as u32;
        pos += 1;
        let negative = first & 0x80 != 0;
        let mut magnitude = (first & 0x3F) as i64;
        let mut shift = 6;
        if first & 0x40 != 0 {
            loop {
                let byte = *self.code.get(pos).ok_or_else(truncated)?;
                pos += 1;
                magnitude |= i64::from(byte & 0x7F) << shift;
                shift += 7;
                if byte & 0x80 == 0 || shift >= 35 {
                    break;
                }
            }
        }

        self.pc = pos;
        Ok(if negative { -magnitude } else { magnitude } as i32)
    }

    fn name_ref(&mut self) -> VmResult<u32> {
        let raw = self.name_ci()?;
        self.resolver.resolve_name(raw)
    }
    /// Decode one compact package object reference through this code stream's
    /// loader-owned resolver.  Unlike `ReadName`, C++ `FFrame::ReadObject`
    /// receives the serialized `UObject*` reference; its number is never a
    /// name-pool index or an arena id.
    fn object_ref(&mut self, operand: &str) -> VmResult<ObjectId> {
        let raw = self.name_ci()?;
        self.resolver.resolve(raw)?.ok_or_else(|| {
            Fail::new(
                "vm.object_ref_null",
                format!("{operand} uses a null object reference"),
            )
        })
    }

    fn property_ref(&mut self, operand: &str) -> VmResult<(ObjectId, u32)> {
        let operand_pc = self.pc;
        let raw = self.name_ci()?;
        let id = self.resolver.resolve(raw)?.ok_or_else(|| {
            Fail::new(
                "vm.object_ref_null",
                format!("{operand} uses a null object reference at pc {operand_pc}"),
            )
        })?;
        let property = self.arena.get(id)?;
        if !matches!(property.data, ObjectData::Property(_)) {
            return Err(Fail::new(
                "vm.object_ref_type",
                format!(
                    "{operand} raw ref {raw} in package {} at pc {operand_pc} resolved to {id:?} ({}) instead of a UProperty",
                    self.resolver.package(),
                    self.arena.path_of(id).unwrap_or_else(|_| "?".to_string())
                ),
            ));
        }
        Ok((id, property.name_index))
    }

    /// Typed zero for a context member expression without evaluating it.
    /// `execContext` memzeros the result buffer when the object is null; for
    /// direct variable members the serialized UProperty operand is the
    /// authoritative declaration for reconstructing that typed value.
    fn context_member_zero(&mut self) -> VmResult<PropValue> {
        let member_pc = self.pc;
        let Some(opcode) = self.code.get(member_pc).copied() else {
            return Err(truncated());
        };
        if !matches!(
            USToken::from_opcode(opcode),
            Some(
                USToken::LocalVariable
                    | USToken::InstanceVariable
                    | USToken::DefaultVariable
                    | USToken::NativeParm
            )
        ) {
            return Ok(PropValue::Int(0));
        }
        self.pc = member_pc + 1;
        let resolved = self
            .property_ref("null context member operand")
            .and_then(|(property, _)| self.arena.default_for_property(property));
        self.pc = member_pc;
        resolved
    }

    fn class_ref(&mut self, operand: &str) -> VmResult<ObjectId> {
        let operand_pc = self.pc;
        let raw = self.name_ci()?;
        let id = self.resolver.resolve(raw)?.ok_or_else(|| {
            Fail::new(
                "vm.object_ref_null",
                format!("{operand} uses a null object reference at pc {operand_pc}"),
            )
        })?;
        if !matches!(self.arena.get(id)?.data, ObjectData::Class(_)) {
            return Err(Fail::new(
                "vm.object_ref_type",
                format!(
                    "{operand} raw ref {raw} in package {} at pc {operand_pc} resolved to {id:?} ({}) instead of a UClass",
                    self.resolver.package(),
                    self.arena.path_of(id).unwrap_or_else(|_| "?".to_string())
                ),
            ));
        }
        Ok(id)
    }

    fn u8_at(&mut self) -> VmResult<u8> {
        let v = *self.code.get(self.pc).ok_or_else(truncated)?;
        self.pc += 1;
        Ok(v)
    }

    /// Execute one token. Returns `true` when `EX_Return` terminated the run.
    fn exec(&mut self, token: USToken) -> VmResult<bool> {
        use PropValue::*;
        match token {
            // ---- Variable loads ------------------------------------------
            USToken::LocalVariable | USToken::NativeParm => {
                let (property, name) = self.property_ref("local/property operand")?;
                self.result = property_value(self.arena, property, self.locals.get(name))?;
            }
            USToken::InstanceVariable => {
                let (property, name) = self.property_ref("instance property operand")?;
                self.result = instance_value(self, property, name)?;
            }
            USToken::DefaultVariable => {
                let (property, name) = self.property_ref("default property operand")?;
                self.result = default_value(self, property, name)?;
            }
            USToken::BoolVariable => {
                // Fork semantics (UnCorSc.cpp `execBoolVariable`): consume
                // ONE byte and re-dispatch `GNatives[B]`; the compiler
                // always writes EX_BoolVariable before a plain variable
                // load or struct-member access (UnScrCom.cpp), so the bool
                // bitmask lives in property metadata, never in code. The
                // nested load's value folds to the result.
                let sub = self.u8_at()?;
                self.expr_opcode(sub)?;
                self.result = Bool(self.result.truthy());
            }

            // ---- Constants ----------------------------------------------
            USToken::IntConst => self.result = Int(self.i32_at()?),
            USToken::IntConstByte => self.result = Int(i32::from(self.u8_at()?)),
            USToken::IntZero => self.result = Int(0),
            USToken::IntOne => self.result = Int(1),
            USToken::ByteConst => self.result = Byte(self.u8_at()?),
            USToken::FloatConst => {
                let raw = self.i32_at()? as u32;
                self.result = Float(f32::from_le_bytes(raw.to_le_bytes()));
            }
            USToken::True => self.result = Bool(true),
            USToken::False => self.result = Bool(false),
            USToken::NameConst => {
                let index = self.name_ref()?;
                self.result = Name(crate::name::Name {
                    index,
                    number: crate::name::NO_NUMBER,
                });
            }
            USToken::ObjectConst => {
                self.result = Object(Some(self.object_ref("object constant")?.0 as i32));
            }
            USToken::NoObject => self.result = Object(None),
            USToken::StringConst => self.result = Str(self.nul_terminated_string()?),
            USToken::UnicodeStringConst => {
                let mut units = Vec::new();
                loop {
                    let lo = self.u8_at()?;
                    let hi = self.u8_at()?;
                    let unit = u16::from_le_bytes([lo, hi]);
                    if unit == 0 {
                        break;
                    }
                    units.push(unit);
                }
                self.result = Str(String::from_utf16_lossy(&units));
            }
            USToken::VectorConst | USToken::RotationConst => {
                let mut raw = [0i32; 3];
                for slot in &mut raw {
                    *slot = self.i32_at()?;
                }
                self.result = if token == USToken::VectorConst {
                    let comps = raw.map(|bits| f32::from_le_bytes(bits.to_le_bytes()));
                    crate::natives::natives_impl::vector_value(self, comps)?
                } else {
                    crate::natives::natives_impl::rotator_value(self, raw)?
                };
            }

            // ---- Assignment & control flow -------------------------------
            USToken::Let | USToken::LetBool => {
                let target = self.pop_lvalue_target()?;
                self.expr()?;
                if self.awaiting_effect_result() || self.suspended_call.is_some() {
                    self.defer_expression(PendingExpression::Assign {
                        target,
                        bool_coerce: token == USToken::LetBool,
                    });
                    return Ok(false);
                }
                let value = self.result.clone();
                let value = if token == USToken::LetBool && !matches!(value, Bool(_)) {
                    Bool(value.truthy())
                } else {
                    value
                };
                self.assign_lvalue(target, value)?;
            }
            USToken::Jump => {
                let runtime_target = usize::from(self.u16_at()?);
                self.pc = self.serialized_offset(runtime_target)?;
            }
            USToken::JumpIfNot => {
                let runtime_target = usize::from(self.u16_at()?);
                self.expr()?;
                if self.awaiting_effect_result() {
                    self.defer_expression(PendingExpression::JumpIfNot { runtime_target });
                    return Ok(false);
                }
                if !self.result.truthy() {
                    self.pc = self.serialized_offset(runtime_target)?;
                }
            }
            USToken::Stop => self.pc = self.code.len(),
            USToken::Nothing => {}
            USToken::Return => {
                self.expr()?;
                if self.awaiting_effect_result() {
                    self.defer_expression(PendingExpression::Return);
                    return Ok(false);
                }
                self.pc = self.code.len();
                return Ok(true);
            }
            USToken::Skip => {
                // EX_Skip is parameter metadata, not a control-flow opcode:
                // C++ natives consume it with `P_GET_SKIP_OFFSET` while
                // deciding whether to evaluate an optional/coerced argument
                // (`UnScript.h:64-68`). Our eager CallArgs collection always
                // evaluates the embedded expression. Translate the span only
                // to validate its runtime coordinate; never jump over later
                // parameters or their EX_EndFunctionParms.
                let runtime_delta = usize::from(self.u16_at()?);
                let _skip_end = self.serialized_relative_target(self.pc, runtime_delta)?;
                self.expr()?;
            }
            USToken::Assert => {
                // Runtime layout (UnClass.cpp `case EX_Assert`): a _WORD
                // line number then the assert expression — no guard byte.
                let _line = self.u16_at()?;
                self.expr()?;
            }
            USToken::GotoLabel => {
                // Compiler emission (UnScrCom.cpp:5036): `Writer <<
                // EX_GotoLabel` then CompileExpr of the label name — a
                // full expression (typically EX_NameConst), not a bare
                // compact index. Function context has no state frame, so
                // like `UObject::GotoLabel` outside a state this cannot
                // jump; consume the operand expression and keep executing.
                self.expr()?;
            }
            USToken::Switch => {
                // `execSwitch`: size byte, switch expression, then a chain
                // of EX_Case records; each carries the address of the next
                // record (MAXWORD = default) plus its compare expression.
                let _size = self.u8_at()?; // raw-width vs string compare
                self.expr()?;
                let switch_value = std::mem::replace(&mut self.result, PropValue::Int(0));
                // The record chain links by absolute offsets; a corrupt
                // stream could loop it forever, so bound iterations by the
                // stream length (every record consumes >= 3 bytes anyway).
                let mut records = 0usize;
                loop {
                    records += 1;
                    if records > self.code.len() {
                        return Err(Fail::new(
                            "vm.switch_chain_unterminated",
                            "EX_Switch case-record chain does not terminate",
                        ));
                    }
                    let peek = *self.code.get(self.pc).ok_or_else(truncated)?;
                    if peek != USToken::Case as u8 {
                        return Err(Fail::new(
                            "vm.switch_case_expected",
                            format!("expected EX_Case at {}, found {peek:#04x}", self.pc),
                        ));
                    }
                    self.pc += 1;
                    let next_runtime = usize::from(self.u16_at()?);
                    if next_runtime == 0xFFFF {
                        break; // default body starts here.
                    }
                    self.expr()?;
                    let case_value = std::mem::replace(&mut self.result, PropValue::Int(0));
                    let matches = match (&switch_value, &case_value) {
                        (PropValue::Str(left), PropValue::Str(right)) => {
                            left.eq_ignore_ascii_case(right)
                        }
                        _ => switch_value == case_value,
                    };
                    if matches {
                        break; // matched body starts here.
                    }
                    self.pc = self.serialized_offset(next_runtime)?;
                }
            }
            USToken::Case => {
                // Reached by falling through an unmatched body: skip this
                // record's compare expression (C-style fallthrough).
                let next = self.u16_at()? as usize;
                if next != 0xFFFF {
                    self.expr()?;
                }
            }
            USToken::LabelTable => {
                // Entries of (name, code offset) terminated by NAME_None,
                // parsed so state payloads stay decodable headlessly.
                let mut entries = 0usize;
                loop {
                    entries += 1;
                    if entries * 5 > self.code.len() + 5 {
                        return Err(Fail::new(
                            "vm.label_table_unterminated",
                            "label table has no NAME_None terminator",
                        ));
                    }
                    let name = self.name_ci()?;
                    let _offset = self.i32_at()?;
                    if name == 0 {
                        break;
                    }
                }
            }
            USToken::DebugInfo => {
                // Runtime layout (UnClass.cpp `case EX_DebugInfo`): INT
                // version + INT line + INT char pos + a NUL-terminated BYTE
                // string. `HANDLE_OPTIONAL_DEBUG_INFO` can emit one after
                // every call site, so it executes as a consumed no-op.
                let _version = self.i32_at()?;
                let _line = self.i32_at()?;
                let _pos = self.i32_at()?;
                let _label = self.nul_terminated_string()?;
            }

            // C++ source: `UnCorSc.cpp` `execFinalFunction` calls
            // `Stack.ReadObject()`, while `execVirtualFunction` and
            // `execGlobalFunction` call `Stack.ReadName()`; `UnClass.cpp`
            // `SerializeExpr` cases EX_FinalFunction/EX_VirtualFunction/
            // EX_GlobalFunction (1608–1624) match those exact operand forms.
            USToken::FinalFunction => {
                let function_id = self.object_ref("EX_FinalFunction")?;
                let callee = self.callee_from_final(function_id)?;
                let member_self = self.self_id;
                self.self_id = self.lexical_self_id;
                let args = self.collect_parms(
                    &callee.params,
                    (callee.native_index != 0).then_some(callee.native_index),
                );
                self.self_id = member_self;
                let args = args?;
                if self.awaiting_effect_result() {
                    self.defer_resolved_call(callee, args);
                    return Ok(false);
                }
                self.dispatch_resolved(&callee, &args)?;
            }
            USToken::VirtualFunction | USToken::GlobalFunction => {
                let selector = self.name_ref()?;
                // Resolve BEFORE the operand walk so parameter names and
                // omitted-argument defaults are available; a resolution
                // failure is reported only after all parms are consumed.
                let found = self.resolve_named_callee(token, selector)?;
                match found {
                    Some(callee) => {
                        let member_self = self.self_id;
                        self.self_id = self.lexical_self_id;
                        let args = self.collect_parms(
                            &callee.params,
                            (callee.native_index != 0).then_some(callee.native_index),
                        );
                        self.self_id = member_self;
                        let args = args?;
                        if self.awaiting_effect_result() {
                            self.defer_resolved_call(callee, args);
                            return Ok(false);
                        }
                        self.dispatch_resolved(&callee, &args)?;
                    }
                    None => {
                        let name = self.arena.names.text(selector.max(0) as u32).unwrap_or("?");
                        self.collect_parms(&[], None)?;
                        return Err(Fail::new(
                            "vm.function_unresolved",
                            format!("{token:?} {name}: no function on the target's class chain"),
                        ));
                    }
                }
            }

            // ---- Member/array access ------------------------------------
            USToken::StructMember => {
                let (field_property, field) = self.property_ref("struct member operand")?;
                let base_pc = self.pc;
                self.expr()?;
                let value = std::mem::replace(&mut self.result, PropValue::Int(0));
                match value {
                    PropValue::Struct {
                        struct_name,
                        fields,
                    } => {
                        let field_text = self.arena.names.text(field).unwrap_or("");
                        let direct = fields
                            .iter()
                            .find(|(name, _)| {
                                self.arena
                                    .names
                                    .text(*name)
                                    .is_some_and(|text| text.eq_ignore_ascii_case(field_text))
                            })
                            .map(|(_, value)| value.clone());
                        self.result = match direct {
                            Some(value) => value,
                            None => self
                                .arena
                                .default_for_struct_member(struct_name, field_property)
                                .map_err(|error| {
                                    let field_names: Vec<String> = fields
                                        .iter()
                                        .map(|(name, _)| {
                                            self.arena
                                                .names
                                                .text(*name)
                                                .unwrap_or("?")
                                                .to_string()
                                        })
                                        .collect();
                                    Fail::new(
                                        "vm.token_unsupported",
                                        format!(
                                            "struct member {} missing on struct {:?} fields={field_names:?}: {}",
                                            self.arena.names.text(field).unwrap_or("?"),
                                            self.arena.names.text(struct_name).unwrap_or("?"),
                                            error.message
                                        ),
                                    )
                                })?,
                        };
                    }
                    other => {
                        let property_path = self
                            .arena
                            .path_of(field_property)
                            .unwrap_or_else(|_| format!("{field_property:?}"));
                        let saved_pc = self.pc;
                        let base_property_path = match self.code.get(base_pc).copied() {
                            Some(opcode @ (0x00 | 0x01 | 0x02 | 0x04 | 0x48))
                                if matches!(
                                    USToken::from_opcode(opcode),
                                    Some(
                                        USToken::LocalVariable
                                            | USToken::InstanceVariable
                                            | USToken::DefaultVariable
                                            | USToken::NativeParm
                                    )
                                ) =>
                            {
                                self.pc = base_pc + 1;
                                self.property_ref("struct base operand")
                                    .ok()
                                    .and_then(|(id, _)| self.arena.path_of(id).ok())
                            }
                            _ => None,
                        };
                        self.pc = saved_pc;
                        let base_bytes = self.code.get(base_pc..saved_pc).unwrap_or(&[]);
                        return Err(Fail::new(
                            "vm.token_unsupported",
                            format!(
                                "struct member {} ({property_path}) on non-struct {other:?} \
                                 from {} at byte {base_pc} {base_bytes:02x?} is deferred",
                                self.arena.names.text(field).unwrap_or("?"),
                                base_property_path
                                    .as_deref()
                                    .unwrap_or("non-property expression")
                            ),
                        ));
                    }
                }
            }
            USToken::ArrayElement | USToken::DynArrayElement => {
                // Two operand expressions; which side carries the array
                // follows from the decoded values (fork streams are
                // consistent, so this is unambiguous in practice).
                self.expr()?;
                let first = std::mem::replace(&mut self.result, PropValue::Int(0));
                self.expr()?;
                let second = std::mem::replace(&mut self.result, PropValue::Int(0));
                let (array, index_value) = match (&first, &second) {
                    (PropValue::Array(_) | PropValue::FixedArray(_), _) => (first, second),
                    (_, PropValue::Array(_) | PropValue::FixedArray(_)) => (second, first),
                    _ => {
                        return Err(Fail::new(
                            "vm.token_unsupported",
                            "element access on a non-array expression is deferred",
                        ));
                    }
                };
                let index = match index_value {
                    PropValue::Byte(b) => i32::from(b),
                    PropValue::Int(i) => i,
                    PropValue::Float(f) => f as i32,
                    _ => 0,
                };
                let items = match array {
                    PropValue::Array(items) | PropValue::FixedArray(items) => items,
                    _ => {
                        return Err(Fail::new(
                            "vm.token_unsupported",
                            "element access on a non-array expression is deferred",
                        ));
                    }
                };
                if index < 0 || index as usize >= items.len() {
                    return Err(Fail::new(
                        "vm.array_out_of_bounds",
                        format!("{token:?}: index {index} outside 0..{}", items.len()),
                    ));
                }
                self.result = items[index as usize].clone();
            }
            USToken::DynArrayCount => {
                self.expr()?;
                self.result = match std::mem::replace(&mut self.result, PropValue::Int(0)) {
                    PropValue::Array(items) | PropValue::FixedArray(items) => {
                        Int(i32::try_from(items.len()).unwrap_or(i32::MAX))
                    }
                    _ => Int(0),
                };
            }
            // C++ source: `UnCorSc.cpp` `execContext`/`execClassContext`
            // evaluates the object expression, then reads `_WORD wSkip` and
            // `BYTE bSize` before one member expression; `UnClass.cpp`
            // `SerializeExpr` cases EX_Context/EX_ClassContext (1631–1638)
            // emits precisely that form.
            USToken::Context | USToken::ClassContext => {
                self.expr()?;
                if self.awaiting_effect_result() {
                    self.defer_expression(PendingExpression::Context {
                        class_context: token == USToken::ClassContext,
                    });
                    return Ok(false);
                }
                let ctx = std::mem::replace(&mut self.result, PropValue::Int(0));
                let skip = usize::from(self.u16_at()?);
                let _zero_fill = self.u8_at()?;
                let target = match ctx {
                    Object(Some(raw)) if raw >= 0 => ObjectId(raw as u32),
                    _ => {
                        self.result = self.context_member_zero()?;
                        // `execContext` advances from the member-expression
                        // boundary by a runtime Script-byte count.
                        let target_pc = self.serialized_relative_target(self.pc, skip)?;
                        self.pc = target_pc;
                        return Ok(false);
                    }
                };
                let member_self = if token == USToken::ClassContext {
                    match &self.arena.get(target)?.data {
                        ObjectData::Class(d) => d.default_object.ok_or_else(|| {
                            Fail::new(
                                "vm.token_unsupported",
                                "ClassContext on a class without a default object is deferred",
                            )
                        })?,
                        _ => {
                            return Err(Fail::new(
                                "vm.object_ref_type",
                                format!("EX_ClassContext target {target:?} is not a UClass"),
                            ));
                        }
                    }
                } else {
                    target
                };
                let saved = self.self_id.replace(member_self);
                let member = self.expr();
                self.self_id = saved;
                member?;
            }
            USToken::MetaCast | USToken::DynamicCast => {
                let class = self.class_ref("cast class operand")?;
                self.expr()?;
                if self.awaiting_effect_result() {
                    self.defer_expression(PendingExpression::Cast { token, class });
                    return Ok(false);
                }
                self.result = cast(self.arena, token, class, &self.result)?;
            }

            // ---- Iterators ----------------------------------------------
            USToken::Iterator => {
                // First execute the iterator native. It captures the output
                // lvalue and candidate set while its argument stream is live.
                self.expr()?;
                if self.awaiting_effect_result() {
                    let (actor, hit_location, hit_normal) =
                        self.pending_trace_iterator.take().ok_or_else(|| {
                            Fail::new(
                                "native.iterator_uninitialized",
                                "result-bearing iterator did not capture its out parameters",
                            )
                        })?;
                    self.defer_expression(PendingExpression::BeginTraceIterator {
                        actor,
                        hit_location,
                        hit_normal,
                    });
                    return Ok(false);
                }
                // The compiler stores an absolute in-memory Script offset
                // here: `EmitAddressToFixupLater` records `Script.Num()` and
                // `PopNest` writes `Fixups[FIXUP_IteratorEnd]` directly into
                // the `_WORD` (UnScrCom.cpp:509-518, 3010-3015, 3051-3075,
                // 4989-5004). `SerializeExpr` expands compact UObject/FName
                // archive operands to four-byte Script slots, so translate
                // that runtime offset back to this serialized code slice.
                let runtime_end = usize::from(self.u16_at()?);
                let end_offset = self.serialized_offset(runtime_end)?;
                let end_pc = end_offset.checked_add(1).ok_or_else(|| {
                    Fail::new("vm.iterator_end_invalid", "iterator end offset overflowed")
                })?;
                if end_pc > self.code.len()
                    || self.code.get(end_offset).copied() != Some(USToken::IteratorPop as u8)
                {
                    return Err(Fail::new(
                        "vm.iterator_end_invalid",
                        format!(
                            "EX_Iterator runtime end {runtime_end} maps to serialized byte {end_offset} on {:?}, expected EX_IteratorPop",
                            self.code.get(end_offset).copied()
                        ),
                    ));
                }
                let pending = self.pending_iterator.take().ok_or_else(|| {
                    Fail::new(
                        "native.iterator_uninitialized",
                        "iterator expression did not initialize a native iterator cursor",
                    )
                })?;
                let body_pc = self.pc;
                let Some(first) = pending.values.first().cloned() else {
                    if pending.clear_actor_on_exhaust {
                        self.assign_lvalue(pending.output, PropValue::Object(None))?;
                    }
                    self.pc = end_pc;
                    return Ok(false);
                };
                self.bind_iterator_item(
                    &first,
                    pending.output.clone(),
                    pending.hit_location.clone(),
                    pending.hit_normal.clone(),
                )?;
                self.iterators.push(IteratorCursor {
                    values: pending.values,
                    index: 0,
                    output: pending.output,
                    hit_location: pending.hit_location,
                    hit_normal: pending.hit_normal,
                    clear_actor_on_exhaust: pending.clear_actor_on_exhaust,
                    body_pc,
                    end_pc,
                });
            }
            USToken::IteratorNext => {
                let next = {
                    let Some(cursor) = self.iterators.last_mut() else {
                        return Err(Fail::new(
                            "vm.iterator_stack_empty",
                            "EX_IteratorNext has no active iterator",
                        ));
                    };
                    cursor.index += 1;
                    cursor.values.get(cursor.index).cloned().map(|item| {
                        (
                            item,
                            cursor.output.clone(),
                            cursor.hit_location.clone(),
                            cursor.hit_normal.clone(),
                            cursor.body_pc,
                        )
                    })
                };
                if let Some((item, output, hit_location, hit_normal, body_pc)) = next {
                    self.bind_iterator_item(&item, output, hit_location, hit_normal)?;
                    self.pc = body_pc;
                } else {
                    let cursor = self.iterators.pop().expect("active iterator checked above");
                    if cursor.clear_actor_on_exhaust {
                        self.assign_lvalue(cursor.output, PropValue::Object(None))?;
                    }
                    self.pc = cursor.end_pc;
                }
            }
            USToken::IteratorPop => {
                if self.iterators.pop().is_none() {
                    return Err(Fail::new(
                        "vm.iterator_stack_empty",
                        "EX_IteratorPop has no active iterator",
                    ));
                }
            }

            // ---- Self & struct comparison -------------------------------
            USToken::SelfToken => {
                self.result = PropValue::Object(self.lexical_self_id.map(|id| id.0 as i32));
            }
            USToken::StructCmpEq | USToken::StructCmpNe => {
                // Loader layout (UnClass.cpp `case EX_StructCmpEq/Ne`) and
                // compiler emission (UnScrCom.cpp:2413): the compared
                // UStruct reference first, then left and right expressions.
                let _struct_ref = self.object_ref("struct comparison operand")?;
                self.expr()?;
                self.expr()?;
                let equal = matches!(token, USToken::StructCmpEq);
                self.result = PropValue::Bool(equal);
            }

            // ---- Misc ---------------------------------------------------
            USToken::LineNumber => {
                // Compiler emission (UnScrCom.cpp: `_WORD wLine =
                // InputLine; Writer << EX_LineNumber; Writer << wLine;`): a
                // _WORD line number, not a compact index.
                let _line = self.u16_at()?;
            }
            USToken::EatString => {
                self.expr()?;
            }
            USToken::New => {
                // Loader walk (UnClass.cpp `case EX_New`) consumes four
                // expressions — parent, name, flags, class — before
                // allocation. Allocation stays deferred, but the operands
                // must be consumed so a deferral keeps streams aligned
                // (same policy as EX_Iterator).
                self.expr()?; // parent
                self.expr()?; // name
                self.expr()?; // flags
                self.expr()?; // class
                return Err(Fail::new(
                    "vm.token_unsupported",
                    "New: object allocation is deferred",
                ));
            }
            USToken::ByteToInt
            | USToken::ByteToBool
            | USToken::ByteToFloat
            | USToken::IntToByte
            | USToken::IntToBool
            | USToken::IntToFloat
            | USToken::BoolToByte
            | USToken::BoolToInt
            | USToken::BoolToFloat
            | USToken::FloatToByte
            | USToken::FloatToInt
            | USToken::FloatToBool
            | USToken::ObjectToBool
            | USToken::NameToBool
            | USToken::StringToByte
            | USToken::StringToInt
            | USToken::StringToBool
            | USToken::StringToFloat
            | USToken::StringToVector
            | USToken::StringToRotator
            | USToken::VectorToBool
            | USToken::VectorToRotator
            | USToken::RotatorToBool
            | USToken::RotatorToVector
            | USToken::ByteToString
            | USToken::IntToString
            | USToken::BoolToString
            | USToken::FloatToString
            | USToken::ObjectToString
            | USToken::NameToString
            | USToken::VectorToString
            | USToken::RotatorToString
            | USToken::StringToName => {
                self.expr()?;
                if self.awaiting_effect_result() {
                    self.defer_expression(PendingExpression::Convert { token });
                    return Ok(false);
                }
                self.result = convert(self.arena, token, &self.result)?;
            }

            // ---- Extended natives ---------------------------------------
            // Fork runtime (`UnCorSc.cpp` / `UnClass.cpp` SerializeExpr):
            // `Expr >= EX_ExtendedNative` transfers exactly ONE extra BYTE
            // and the native id spans `(Expr - EX_ExtendedNative)*0x100 +
            // B`, giving slots 0..0xFFF across opcodes 0x60..0x6F. The
            // parameter list follows up to `EX_EndFunctionParms`.
            USToken::ExtendedNative => {
                let low = self.u8_at()?;
                let slot = u16::from(low);
                let args = self.collect_parms(&[], Some(slot))?;
                if self.awaiting_effect_result() {
                    self.defer_native_call(slot, Vec::new(), args);
                    return Ok(false);
                }
                self.result = self.invoke_native(slot, &args)?;
            }
            USToken::FirstNative => {
                return Err(Fail::new(
                    "vm.token_unsupported",
                    "FirstNative: numbered natives dispatch via ExtendedNative",
                ));
            }

            // Tokens that terminate parameter lists are never executed.
            USToken::EndFunctionParms => {
                return Err(Fail::new(
                    "vm.token_unexpected_end_parms",
                    "EndFunctionParms encountered outside a parameter list",
                ));
            }
        }
        Ok(false)
    }

    /// Evaluate one expression element into `self.result`.
    fn expr(&mut self) -> VmResult<()> {
        let opcode = *self.code.get(self.pc).ok_or_else(truncated)?;
        self.pc += 1;
        self.expr_opcode(opcode)
    }

    /// Dispatch one already-consumed expression opcode. Shared by
    /// [`Frame::expr`] and tokens that embed a nested expression opcode in
    /// their stream (the fork's `EX_BoolVariable`).
    fn expr_opcode(&mut self, opcode: u8) -> VmResult<()> {
        if (0x61..=0x6F).contains(&opcode) {
            let low = self.u8_at()?;
            let slot = u16::from(opcode - 0x60) * 0x100 + u16::from(low);
            let args = self.collect_parms(&[], Some(slot))?;
            if self.awaiting_effect_result() {
                self.defer_native_call(slot, Vec::new(), args);
                return Ok(());
            }
            self.result = self.invoke_native(slot, &args)?;
            return Ok(());
        }
        if (0x70..=0xFF).contains(&opcode) {
            return self.dispatch_native(u16::from(opcode));
        }
        let Some(token) = USToken::from_opcode(opcode) else {
            return Err(unknown_token(opcode, self.pc - 1));
        };
        self.exec(token)?;
        Ok(())
    }

    fn pop_lvalue_target(&mut self) -> VmResult<LvalueTarget> {
        self.capture_lvalue().map(|(target, _)| target)
    }

    /// Decode one UE address expression and read its current value. This is
    /// the Rust equivalent of the `P_GET_*_REF` macros in
    /// `Core/Src/UnCorSc.cpp`: the first parameter is stepped as an address,
    /// while later `P_GET_*` parameters are evaluated as values.
    /// Preserve address metadata for legacy iterator/output consumers while
    /// ordinary arguments still follow the value ABI. Compound slots do not
    /// use this probe: they call `capture_lvalue` and never evaluate a
    /// temporary first.
    fn simple_lvalue_at(&self, at: usize) -> Option<LValue> {
        let opcode = *self.code.get(at)?;
        let (raw, _) = compact_index_at(&self.code, at + 1)?;
        let property = self.resolver.resolve(raw).ok().flatten()?;
        let name = self.arena.get(property).ok()?.name_index;
        match USToken::from_opcode(opcode) {
            Some(USToken::LocalVariable | USToken::NativeParm) => Some(LValue::Local(name)),
            Some(USToken::InstanceVariable) => Some(LValue::Instance {
                actor: self.self_id?,
                name,
            }),
            Some(USToken::DefaultVariable) => Some(LValue::Default {
                object: self.default_object_for_self().ok()?,
                name,
            }),
            _ => None,
        }
    }

    fn capture_lvalue(&mut self) -> VmResult<(LValue, PropValue)> {
        let opcode = *self.code.get(self.pc).ok_or_else(truncated)?;
        self.pc += 1;
        self.capture_lvalue_opcode(opcode)
    }

    fn capture_lvalue_opcode(&mut self, opcode: u8) -> VmResult<(LValue, PropValue)> {
        match USToken::from_opcode(opcode) {
            Some(USToken::BoolVariable) => {
                let nested = self.u8_at()?;
                self.capture_lvalue_opcode(nested)
                    .map(|(target, value)| (target, PropValue::Bool(value.truthy())))
            }
            Some(USToken::LocalVariable | USToken::NativeParm) => {
                let (property, name) = self.property_ref("local lvalue operand")?;
                let value = property_value(self.arena, property, self.locals.get(name))?;
                if self.locals.get(name).is_none() {
                    self.locals.set(name, value.clone());
                }
                Ok((LValue::Local(name), value))
            }
            Some(USToken::InstanceVariable) => {
                let (property, name) = self.property_ref("instance lvalue operand")?;
                let actor = self.self_id.ok_or_else(|| {
                    Fail::new(
                        "vm.lvalue_unsupported",
                        "instance-variable lvalue with no executing object",
                    )
                })?;
                let value = instance_value(self, property, name)?;
                Ok((LValue::Instance { actor, name }, value))
            }
            Some(USToken::DefaultVariable) => {
                let (property, name) = self.property_ref("default lvalue operand")?;
                let object = self.default_object_for_self()?;
                let value = default_value(self, property, name)?;
                Ok((LValue::Default { object, name }, value))
            }
            Some(USToken::ArrayElement | USToken::DynArrayElement) => {
                let index_pc = self.pc;
                self.expr()?;
                let index = numeric_index(&self.result)?;
                let saved_pc = self.pc;
                let index_property_path = match self.code.get(index_pc).copied() {
                    Some(opcode)
                        if matches!(
                            USToken::from_opcode(opcode),
                            Some(
                                USToken::LocalVariable
                                    | USToken::InstanceVariable
                                    | USToken::DefaultVariable
                                    | USToken::NativeParm
                            )
                        ) =>
                    {
                        self.pc = index_pc + 1;
                        self.property_ref("array index operand")
                            .ok()
                            .and_then(|(id, _)| self.arena.path_of(id).ok())
                    }
                    _ => None,
                };
                self.pc = saved_pc;
                let base_pc = self.pc;
                let (base, value) = self.capture_lvalue()?;
                let base_name = match &base {
                    LValue::Local(name)
                    | LValue::Instance { name, .. }
                    | LValue::Default { name, .. } => self.arena.names.text(*name),
                    _ => None,
                };
                let index_bytes = self.code.get(index_pc..base_pc).unwrap_or(&[]);
                let base_bytes = self.code.get(base_pc..self.pc).unwrap_or(&[]);
                let item = array_item(&value, index).map_err(|error| {
                    Fail::new(
                        error.reason_code,
                        format!(
                            "{}; base={base:?} ({base_name:?}), index {} at byte \
                             {index_pc} {index_bytes:02x?}, base expression at byte {base_pc} \
                             {base_bytes:02x?}",
                            error.message,
                            index_property_path.as_deref().unwrap_or("expression")
                        ),
                    )
                })?;
                Ok((
                    LValue::ArrayElement {
                        base: Box::new(base),
                        index,
                    },
                    item.clone(),
                ))
            }
            Some(USToken::StructMember) => {
                let (field_property, field) = self.property_ref("struct lvalue field")?;
                let (base, value) = self.capture_lvalue()?;
                let PropValue::Struct {
                    struct_name,
                    fields,
                } = value
                else {
                    return Err(Fail::new(
                        "vm.lvalue_unsupported",
                        "struct-field lvalue has a non-struct base",
                    ));
                };
                let value = fields
                    .into_iter()
                    .find(|(name, _)| *name == field)
                    .map(|(_, value)| value)
                    .map(Ok)
                    .unwrap_or_else(|| {
                        self.arena
                            .default_for_struct_member(struct_name, field_property)
                    })?;
                Ok((
                    LValue::StructField {
                        base: Box::new(base),
                        field,
                        property: field_property,
                    },
                    value,
                ))
            }
            Some(USToken::Context | USToken::ClassContext) => {
                self.expr()?;
                let context = std::mem::replace(&mut self.result, PropValue::Int(0));
                let _skip = self.u16_at()?;
                let _zero_fill = self.u8_at()?;
                let target = match context {
                    PropValue::Object(Some(raw)) if raw >= 0 => ObjectId(raw as u32),
                    other => {
                        return Err(Fail::new(
                            "vm.lvalue_unsupported",
                            format!("context lvalue has no object target: {other:?}"),
                        ));
                    }
                };
                let member_self = if opcode == USToken::ClassContext as u8 {
                    match &self.arena.get(target)?.data {
                        ObjectData::Class(data) => data.default_object.ok_or_else(|| {
                            Fail::new(
                                "vm.lvalue_unsupported",
                                "class-context lvalue class has no default object",
                            )
                        })?,
                        _ => {
                            return Err(Fail::new(
                                "vm.object_ref_type",
                                format!("class-context lvalue target {target:?} is not a class"),
                            ));
                        }
                    }
                } else {
                    target
                };
                let saved = self.self_id.replace(member_self);
                let captured = self.capture_lvalue();
                self.self_id = saved;
                captured
            }
            _ => Err(Fail::new(
                "vm.lvalue_unsupported",
                format!("opcode {opcode:#04x} cannot be used as an out parameter"),
            )),
        }
    }

    pub fn read_lvalue(&self, target: &LValue) -> VmResult<PropValue> {
        match target {
            LValue::Local(name) => self.locals.get(*name).cloned().ok_or_else(|| {
                Fail::new(
                    "vm.lvalue_uninitialized",
                    format!("local {name} has no value"),
                )
            }),
            LValue::Instance { actor, name } => {
                let class_id = self.arena.get(*actor).ok().and_then(|value| value.class_id);
                self.lvalue_property_value(*actor, *name, class_id)
            }
            LValue::Default { object, name } => {
                let owner_class = self.arena.get(*object).ok().and_then(|value| {
                    value.outer.filter(|outer| {
                        self.arena
                            .get(*outer)
                            .is_ok_and(|owner| matches!(owner.data, ObjectData::Class(_)))
                    })
                });
                self.lvalue_property_value(*object, *name, owner_class)
            }
            LValue::ArrayElement { base, index } => {
                Ok(array_item(&self.read_lvalue(base)?, *index)?.clone())
            }
            LValue::StructField {
                base,
                field,
                property,
            } => {
                let PropValue::Struct {
                    struct_name,
                    fields,
                } = self.read_lvalue(base)?
                else {
                    return Err(Fail::new(
                        "vm.lvalue_unsupported",
                        "struct-field lvalue has a non-struct base",
                    ));
                };
                fields
                    .into_iter()
                    .find(|(name, _)| name == field)
                    .map(|(_, value)| value)
                    .map(Ok)
                    .unwrap_or_else(|| self.arena.default_for_struct_member(struct_name, *property))
            }
        }
    }

    pub fn write_lvalue(&mut self, target: &LValue, value: PropValue) -> VmResult<()> {
        match target {
            LValue::Local(name) => self.locals.set(*name, value),
            LValue::Instance { actor, name }
            | LValue::Default {
                object: actor,
                name,
            } => {
                self.queue_write(InstanceWrite {
                    object: *actor,
                    name: *name,
                    value,
                })?;
            }
            LValue::ArrayElement { base, index } => {
                let mut container = self.read_lvalue(base)?;
                let item = array_item_mut(&mut container, *index)?;
                *item = value;
                self.write_lvalue(base, container)?;
            }
            LValue::StructField {
                base,
                field,
                property: _,
            } => {
                let mut container = self.read_lvalue(base)?;
                let PropValue::Struct { fields, .. } = &mut container else {
                    return Err(Fail::new(
                        "vm.lvalue_unsupported",
                        "struct-field lvalue has a non-struct base",
                    ));
                };
                if let Some(member) = fields
                    .iter_mut()
                    .find(|(name, _)| name == field)
                    .map(|(_, member)| member)
                {
                    *member = value;
                } else {
                    // Zero-valued declared struct members may be omitted from
                    // serialized/default bags; assigning one materializes it.
                    fields.push((*field, value));
                }
                self.write_lvalue(base, container)?;
            }
        }
        Ok(())
    }

    fn assign_lvalue(&mut self, target: LvalueTarget, value: PropValue) -> VmResult<()> {
        self.write_lvalue(&target, value)
    }

    /// Dispatch a numbered-native call: evaluate the parameter list up to
    /// `EX_EndFunctionParms` into a positional [`CallArgs`], then run the
    /// registered slot body.
    fn dispatch_native(&mut self, slot: u16) -> VmResult<()> {
        let args = self.collect_parms(&[], Some(slot))?;
        if self.awaiting_effect_result() {
            self.defer_native_call(slot, Vec::new(), args);
            return Ok(());
        }
        self.result = self.invoke_native(slot, &args)?;
        Ok(())
    }

    fn invoke_native(&mut self, slot: u16, args: &CallArgs) -> VmResult<PropValue> {
        let body = self.registry.get(slot)?;
        let previous = self.current_native_slot.replace(slot);
        let result = body(self, args);
        self.current_native_slot = previous;
        result
    }

    /// Native ABI diagnostic context for a body currently dispatched by this
    /// frame. The subject is the exact path installed in the registry.
    pub fn current_native(&self) -> Option<(u16, &str)> {
        let slot = self.current_native_slot?;
        Some((
            slot,
            self.registry.subject(slot).unwrap_or("<unregistered>"),
        ))
    }

    /// Consume parameters up to `EX_EndFunctionParms`, evaluating each
    /// operand expression into the returned argument vector. When the
    /// callee's parameter list is known (`params` non-empty), arguments
    /// carry their declared parameter name and omitted trailing arguments
    /// are padded with defaults (class default object value when
    /// resolvable, else the property-kind zero).
    fn collect_parms(
        &mut self,
        params: &[ObjectId],
        native_slot: Option<u16>,
    ) -> VmResult<CallArgs> {
        let mut args = CallArgs::default();
        let parms_start = self.pc;
        let mut supplied = 0usize;
        loop {
            if supplied >= PARM_PROGRESS_BUDGET {
                return Err(Fail::new(
                    "vm.parm_budget_exceeded",
                    format!("call exceeded {PARM_PROGRESS_BUDGET} evaluated parameters"),
                ));
            }
            let peek = *self.code.get(self.pc).ok_or_else(truncated)?;
            if peek == USToken::EndFunctionParms as u8 {
                self.pc += 1;
                break;
            }
            let before = self.pc;
            let name = params
                .get(supplied)
                .and_then(|p| self.arena.get(*p).ok())
                .map(|o| o.name_index);
            let wants_lvalue = params
                .get(supplied)
                .is_some_and(|param| self.param_is_writable_ref(*param))
                || native_slot.is_some_and(|slot| native_arg_is_lvalue(slot, supplied));
            let (value, lvalue) = if wants_lvalue {
                let (target, value) = self.capture_lvalue()?;
                (value, Some(target))
            } else {
                let lvalue = self.simple_lvalue_at(before);
                self.expr()?;
                (self.result.clone(), lvalue)
            };
            if self.awaiting_effect_result() {
                self.suspended_call_arg = Some((name, lvalue));
                return Ok(args);
            }
            if self.pc <= before {
                return Err(Fail::new(
                    "vm.parm_progress_stalled",
                    format!(
                        "parameter list from {parms_start} moved pc from {before} back to {}; code={:?}",
                        self.pc, &self.code
                    ),
                ));
            }
            args.push(CallArg {
                name,
                value,
                lvalue,
            });
            supplied += 1;
        }
        for param in params.iter().skip(supplied) {
            args.push(CallArg {
                name: self.arena.get(*param).ok().map(|o| o.name_index),
                value: self.default_param_value(*param)?,
                lvalue: None,
            });
        }
        self.consume_optional_debug_info()?;
        Ok(args)
    }

    /// `HANDLE_OPTIONAL_DEBUG_INFO` follows every compiled call in this
    /// fork (`UnClass.cpp` SerializeExpr for Final/Virtual/Global calls).
    /// It belongs to the call expression, not to the enclosing bytecode
    /// stream; leaving it behind makes the next operand (notably an
    /// EX_Iterator relative end offset) start in debug metadata.
    fn consume_optional_debug_info(&mut self) -> VmResult<()> {
        if self.code.get(self.pc).copied() == Some(USToken::DebugInfo as u8) {
            self.pc += 1;
            self.exec(USToken::DebugInfo)?;
        }
        Ok(())
    }

    /// Default for an omitted argument: class-default-object value of the
    /// parameter's name (leaf-first over the executing class chain), else
    /// the zero value of the parameter's declared kind.
    fn default_param_value(&self, param: ObjectId) -> VmResult<PropValue> {
        let param_object = self.arena.get(param)?;
        let param_name = param_object.name_index;
        if let Some(self_id) = self.self_id {
            if let Some(class_id) = self.arena.get(self_id).ok().and_then(|o| o.class_id) {
                for class in self.arena.class_chain(class_id)? {
                    let default_object = match &self.arena.get(class)?.data {
                        ObjectData::Class(d) => d.default_object,
                        _ => None,
                    };
                    if let Some(store) = default_object
                        .and_then(|id| arena_properties(self.arena, id))
                        .and_then(|s| s.get(param_name))
                    {
                        return Ok(store.clone());
                    }
                }
            }
        }
        self.arena.default_for_property(param)
    }

    fn param_is_writable_ref(&self, param: ObjectId) -> bool {
        // UE1 EPropertyFlags: CPF_Const=0x2 and CPF_OutParm=0x100.
        const CPF_CONST: u32 = 0x0000_0002;
        const CPF_OUT_PARM: u32 = 0x0000_0100;
        matches!(
            self.arena.get(param).ok().map(|object| &object.data),
            Some(ObjectData::Property(data))
                if data.property_flags & CPF_OUT_PARM != 0
                    && data.property_flags & CPF_CONST == 0
        )
    }

    fn overlay_value(&self, object: ObjectId, name: u32) -> Option<PropValue> {
        self.pending_writes
            .iter()
            .rev()
            .chain(self.inherited_writes.iter().rev())
            .find(|write| write.object == object && write.name == name)
            .map(|write| write.value.clone())
    }

    fn default_object_for_self(&self) -> VmResult<ObjectId> {
        let class_id = self
            .self_id
            .and_then(|id| self.arena.get(id).ok()?.class_id)
            .ok_or_else(|| {
                Fail::new(
                    "vm.lvalue_unsupported",
                    "default-variable lvalue with no executing class",
                )
            })?;
        self.arena
            .class_chain(class_id)?
            .into_iter()
            .find_map(|class| match &self.arena.get(class).ok()?.data {
                ObjectData::Class(data) => data.default_object,
                _ => None,
            })
            .ok_or_else(|| {
                Fail::new(
                    "vm.lvalue_unsupported",
                    "default-variable lvalue: class has no default object",
                )
            })
    }
    fn lvalue_property_value(
        &self,
        object: ObjectId,
        name: u32,
        class_id: Option<ObjectId>,
    ) -> VmResult<PropValue> {
        let chain = class_id
            .map(|class| self.arena.class_chain(class))
            .transpose()?
            .unwrap_or_default();
        let property = chain.iter().find_map(|class| {
            self.arena
                .children_of(*class)
                .find(|(_, child)| {
                    child.name_index == name && matches!(child.data, ObjectData::Property(_))
                })
                .map(|(id, _)| id)
        });
        let normalize = |value: &PropValue| {
            property
                .map(|property| property_value(self.arena, property, Some(value)))
                .unwrap_or_else(|| Ok(value.clone()))
        };

        if let Some(value) = self.overlay_value(object, name).or_else(|| {
            arena_properties(self.arena, object)
                .and_then(|store| store.get(name))
                .cloned()
        }) {
            return normalize(&value);
        }
        for class in &chain {
            let default_object = match &self.arena.get(*class)?.data {
                ObjectData::Class(data) => data.default_object,
                _ => None,
            };
            if let Some(default_object) = default_object
                && let Some(value) = self.overlay_value(default_object, name).or_else(|| {
                    arena_properties(self.arena, default_object)
                        .and_then(|store| store.get(name))
                        .cloned()
                })
            {
                return normalize(&value);
            }
        }
        if let Some(property) = property {
            return self.arena.default_for_property(property);
        }
        Err(Fail::new(
            "vm.lvalue_uninitialized",
            format!("object {object:?} property {name} has no value or declaration"),
        ))
    }

    /// Resolve the NAME operand of `EX_VirtualFunction`/`EX_GlobalFunction`.
    fn resolve_named_callee(
        &self,
        token: USToken,
        selector: u32,
    ) -> VmResult<Option<ResolvedCallee>> {
        let Some(name) = self.arena.names.text(selector) else {
            return Ok(None);
        };
        if token == USToken::VirtualFunction
            && self.self_id == self.active_state_owner
            && let Some(state_id) = self.active_state_object
            && let Some(function_id) = self.find_state_function(state_id, name)
        {
            return Ok(Some(self.callee_from(function_id)?));
        }
        let class_id = self
            .self_id
            .and_then(|id| self.arena.get(id).ok()?.class_id);
        let Some(class_id) = class_id else {
            return Ok(None);
        };
        self.arena
            .find_function(class_id, name)?
            .map(|function_id| self.callee_from(function_id))
            .transpose()
    }

    fn find_state_function(&self, mut state_id: ObjectId, name: &str) -> Option<ObjectId> {
        let wanted = crate::name::fold_key(name);
        loop {
            if let Some(function) = self
                .arena
                .children_of(state_id)
                .find(|(_, object)| {
                    matches!(object.data, ObjectData::Function(_))
                        && self
                            .arena
                            .names
                            .text(object.name_index)
                            .is_some_and(|actual| crate::name::fold_key(actual) == wanted)
                })
                .map(|(id, _)| id)
            {
                return Some(function);
            }
            state_id = match &self.arena.get(state_id).ok()?.data {
                ObjectData::State(state) => state.links.super_field?,
                _ => return None,
            };
        }
    }

    fn callee_from_final(&self, function_id: ObjectId) -> VmResult<ResolvedCallee> {
        if !matches!(self.arena.get(function_id)?.data, ObjectData::Function(_)) {
            return Err(Fail::new(
                "vm.function_ref_not_function",
                format!("EX_FinalFunction reference {function_id:?} is not a UFunction"),
            ));
        }
        self.callee_from(function_id)
    }

    fn callee_from(&self, function_id: ObjectId) -> VmResult<ResolvedCallee> {
        let (params, native_index) = match &self.arena.get(function_id)?.data {
            ObjectData::Function(f) => (f.params.clone(), f.native_index),
            _ => (Vec::new(), 0),
        };
        Ok(ResolvedCallee {
            function_id,
            params,
            native_index,
        })
    }

    fn dispatch_resolved(&mut self, callee: &ResolvedCallee, args: &CallArgs) -> VmResult<()> {
        let (native_index, function_flags, code, resolver, declared_locals) =
            match &self.arena.get(callee.function_id)?.data {
                ObjectData::Function(f) => (
                    f.native_index,
                    f.function_flags,
                    f.code.clone(),
                    f.resolver.clone(),
                    f.locals.clone(),
                ),
                _ => {
                    return Err(Fail::new(
                        "vm.function_unresolved",
                        "callee object is not a function",
                    ));
                }
            };
        let dispatch_path = self.arena.path_of(callee.function_id).unwrap_or_default();
        let pending_follow_spline =
            if dispatch_path.eq_ignore_ascii_case("Engine.Pawn.CutCommand_FollowSpline")
                && let Some(actor) = self.self_id
                && let Some(PropValue::Str(command)) = args.get(0)
            {
                let cue = match args.get(1) {
                    Some(PropValue::Str(cue)) => cue.clone(),
                    _ => String::new(),
                };
                Some(ScriptEffect::FollowSpline {
                    actor,
                    command: command.clone(),
                    cue,
                })
            } else {
                None
            };
        if self.cutscript_disk_endpoint(callee, args)? {
            return Ok(());
        }
        if native_index != 0 {
            self.result = self.invoke_native(native_index, args)?;
            return Ok(());
        }
        // `IMPLEMENT_FUNCTION(..., INDEX_NONE, ...)` natives (notably
        // UObject::execLocalize) have FUNC_Native but no GNatives slot.
        const FUNC_NATIVE: u32 = 0x0000_0400;
        if function_flags & FUNC_NATIVE != 0 {
            let subject = self
                .arena
                .path_of(callee.function_id)
                .unwrap_or_else(|_| format!("{:?}", callee.function_id));
            if let Some(body) = crate::natives::natives_impl::lookup(&subject) {
                self.result = body(self, args)?;
                return Ok(());
            }
        }
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(Fail::new(
                "vm.call_depth_exceeded",
                format!("script recursion exceeded {MAX_CALL_DEPTH} frames"),
            ));
        }
        let mut nested =
            Frame::with_resolver(self.arena, self.registry, &code, self.self_id, resolver);
        nested.call_depth = self.call_depth + 1;
        nested.rng_state = self.rng_state;
        nested.active_state = self.active_state;
        nested.active_state_object = self.active_state_object;
        nested.active_state_owner = self.active_state_owner;
        nested.actor_scope = self.actor_scope.clone();
        nested.next_effect_request_id = self.next_effect_request_id;
        nested.reserved_object_count = self.reserved_object_count;
        nested.inherited_writes = self.inherited_writes.clone();
        nested.pending_follow_spline = pending_follow_spline;
        nested
            .inherited_writes
            .extend(self.pending_writes.iter().cloned());
        for local in declared_locals {
            let name = self.arena.get(local)?.name_index;
            nested
                .locals
                .set(name, self.arena.default_for_property(local)?);
        }
        for (index, param) in callee.params.iter().enumerate() {
            let Some(value) = args.get(index) else {
                continue;
            };
            let name = self.arena.get(*param)?.name_index;
            nested.locals.set(name, value.clone());
        }
        let mut outcome = nested.run();
        // GotoState changes the actor's state at the engine mutation
        // boundary, but it is an ordinary bool-returning intrinsic: source
        // statements after it in the current function still execute. This
        // matters for Harry.CutCommand("CAPTURE"), whose authored `return
        // true` follows GotoState('stateCutIdle').
        while nested.suspend == Some(Suspend::StateChange) {
            nested.take_suspend();
            outcome?;
            outcome = nested.run();
        }
        if nested.suspend.is_none() && outcome.is_ok() {
            nested.finalize_pending_follow_spline();
        }
        self.absorb_nested_effects(&mut nested);
        if nested.suspend.is_some() {
            // Parent `pc` is already past the call, but it must not run
            // there until this exact callee activation completes after its
            // wake. Persist its full locals/cursor (and any deeper call).
            self.suspended_call = Some(Box::new(nested.snapshot_suspended_call()));
            if self.suspend.is_none() {
                self.suspend = nested.take_suspend();
            }
            outcome?;
            return Ok(());
        }
        outcome?;
        for (index, param) in callee.params.iter().enumerate() {
            let Some(target) = args.lvalue(index).cloned() else {
                continue;
            };
            let name = self.arena.get(*param)?.name_index;
            if let Some(value) = nested.locals.get(name).cloned() {
                self.assign_lvalue(target, value)?;
            }
        }
        self.result = nested.result;
        Ok(())
    }

    /// `CutScriptDisk.GetNextLine` has an authored (commented-out) endpoint
    /// guard but otherwise indexes before testing the returned string. Keep
    /// the fixed-array upper bound exclusive even when all 4096 slots are
    /// populated: return false and clear the out line without evaluating
    /// `lineArray[4096]`.
    fn cutscript_disk_endpoint(
        &mut self,
        callee: &ResolvedCallee,
        args: &CallArgs,
    ) -> VmResult<bool> {
        let path = self.arena.path_of(callee.function_id).unwrap_or_default();
        if !path.eq_ignore_ascii_case("HGame.CutScriptDisk.GetNextLine") {
            return Ok(false);
        }
        let Some(actor) = self.self_id else {
            return Ok(false);
        };
        let class = self.arena.get(actor)?.class_id.ok_or_else(|| {
            Fail::new(
                "vm.object_ref_type",
                "CutScriptDisk.GetNextLine self has no class",
            )
        })?;
        let property = |wanted: &str| -> VmResult<ObjectId> {
            self.arena
                .class_chain(class)?
                .into_iter()
                .find_map(|class| {
                    self.arena.children_of(class).find_map(|(id, object)| {
                        (matches!(object.data, ObjectData::Property(_))
                            && self
                                .arena
                                .names
                                .text(object.name_index)
                                .is_some_and(|name| name.eq_ignore_ascii_case(wanted)))
                        .then_some(id)
                    })
                })
                .ok_or_else(|| {
                    Fail::new(
                        "vm.object_ref_unresolved",
                        format!("CutScriptDisk has no {wanted} property"),
                    )
                })
        };
        let cursor_property = property("curScriptLine")?;
        let cursor_name = self.arena.get(cursor_property)?.name_index;
        let cursor = match instance_value(self, cursor_property, cursor_name)? {
            PropValue::Int(value) => value,
            other => {
                return Err(Fail::new(
                    "vm.lvalue_index_type",
                    format!("CutScriptDisk.curScriptLine is not Int: {other:?}"),
                ));
            }
        };
        let array_property = property("lineArray")?;
        let array_name = self.arena.get(array_property)?.name_index;
        let len = match instance_value(self, array_property, array_name)? {
            PropValue::Array(items) | PropValue::FixedArray(items) => items.len(),
            other => {
                return Err(Fail::new(
                    "vm.lvalue_unsupported",
                    format!("CutScriptDisk.lineArray is not an array: {other:?}"),
                ));
            }
        };
        if cursor < 0 {
            return Err(Fail::new(
                "vm.array_out_of_bounds",
                format!("CutScriptDisk.curScriptLine {cursor} is negative"),
            ));
        }
        if usize::try_from(cursor).is_ok_and(|cursor| cursor < len) {
            return Ok(false);
        }
        let output = args.lvalue(0).cloned().ok_or_else(|| {
            Fail::new(
                "vm.lvalue_unsupported",
                "CutScriptDisk.GetNextLine Line out parameter has no lvalue",
            )
        })?;
        self.write_lvalue(&output, PropValue::Str(String::new()))?;
        self.result = PropValue::Bool(false);
        Ok(true)
    }

    /// Queue one deferred write; a runaway hot loop must not grow the
    /// queue without bound, so exceeding the budget is a loud deferral.
    fn queue_write(&mut self, write: InstanceWrite) -> VmResult<()> {
        if self.pending_writes.len() >= WRITE_QUEUE_BUDGET {
            return Err(Fail::new(
                "vm.write_budget_exceeded",
                format!("deferred-write queue exceeded {WRITE_QUEUE_BUDGET} entries"),
            ));
        }
        self.pending_writes.push(write);
        Ok(())
    }

    /// Read one argument by declared parameter name.
    pub fn arg_named<'b>(&'b self, args: &'b CallArgs, name: &str) -> Option<&'b PropValue> {
        let index = self.arena.names.find_index(name)?;
        args.iter()
            .find(|a| a.name == Some(index))
            .map(|a| &a.value)
    }

    /// Next deterministic pseudo-random `u32` from this frame's LCG.
    pub fn next_rand_u32(&mut self) -> u32 {
        self.rng_state = self
            .rng_state
            .wrapping_mul(1664525)
            .wrapping_add(1013904223);
        self.rng_state
    }

    /// The active suspension, if a latent body parked this frame.
    pub fn suspended(&self) -> Option<Suspend> {
        self.suspend
    }

    /// Take the suspension marker, leaving the frame resumable-clean.
    pub fn take_suspend(&mut self) -> Option<Suspend> {
        self.suspend.take()
    }

    /// Take the parked script-callee activation, if the current frame
    /// yielded from inside one. Embedders persist it beside this frame and
    /// pass it back through [`Frame::restore_suspended_call`] on wake.
    pub fn take_suspended_call(&mut self) -> Option<SuspendedCall> {
        self.suspended_call.take().map(|call| *call)
    }

    /// Restore a parked script-callee activation before resuming this
    /// frame's own cursor.
    pub fn restore_suspended_call(&mut self, call: SuspendedCall) {
        self.suspended_call = Some(Box::new(call));
    }

    /// Drain every queued scheduler request (`SetTimer`/`GotoState`).
    pub fn take_latent_requests(&mut self) -> Vec<LatentRequest> {
        std::mem::take(&mut self.latent_requests)
    }

    /// Queue one engine-facing native effect. Effects must be applied by the
    /// embedding engine after the frame slice, never by a native body.
    pub fn effect(&mut self, effect: ScriptEffect) -> VmResult<()> {
        if self.effects.len() >= EFFECT_QUEUE_BUDGET {
            return Err(Fail::new(
                "vm.effect_budget_exceeded",
                format!("effect queue exceeded {EFFECT_QUEUE_BUDGET} entries"),
            ));
        }
        self.effects.push(effect);
        Ok(())
    }

    /// Retain only a bounded number of structured script diagnostics per
    /// execution slice; ordinary log presentation is independently bounded.
    pub fn log_effect(&mut self, message: String) -> VmResult<()> {
        const LOG_EFFECT_BUDGET: usize = 256;
        if self
            .effects
            .iter()
            .filter(|effect| matches!(effect, ScriptEffect::Log { .. }))
            .count()
            < LOG_EFFECT_BUDGET
        {
            self.effect(ScriptEffect::Log { message })?;
        }
        Ok(())
    }
    /// Reserve one deterministic engine-result request id without reserving
    /// an arena object (metadata queries return scalar values).
    pub fn reserve_effect_request(&mut self) -> VmResult<u64> {
        let request_id = self.next_effect_request_id;
        self.next_effect_request_id =
            self.next_effect_request_id.checked_add(1).ok_or_else(|| {
                Fail::new(
                    "vm.effect_request_id_overflow",
                    "effect request id overflow",
                )
            })?;
        Ok(request_id)
    }

    /// Reserve the append-only arena handle and deterministic request id for
    /// an engine-allocated object.
    pub fn reserve_effect_object(&mut self) -> VmResult<(u64, ObjectId)> {
        let request_id = self.reserve_effect_request()?;
        let raw = u32::try_from(self.arena.len())
            .ok()
            .and_then(|base| base.checked_add(self.reserved_object_count))
            .ok_or_else(|| {
                Fail::new(
                    "native.spawn_allocation_failed",
                    "object arena handle overflow",
                )
            })?;
        self.reserved_object_count =
            self.reserved_object_count.checked_add(1).ok_or_else(|| {
                Fail::new(
                    "native.spawn_allocation_failed",
                    "object reservation count overflow",
                )
            })?;
        Ok((request_id, ObjectId(raw)))
    }

    pub fn await_effect_result(&mut self, request_id: u64) {
        self.pending_effect_request = Some(request_id);
        self.suspend_for(Suspend::EffectResult { request_id });
    }

    /// Replace the unavailable native return in the exact activation that
    /// issued `request_id`, including a deeply nested named call.
    pub fn inject_effect_result(&mut self, request_id: u64, value: PropValue) -> VmResult<()> {
        fn inject(call: &mut SuspendedCall, request_id: u64, value: &PropValue) -> bool {
            if let Some(nested) = call.nested.as_deref_mut()
                && inject(nested, request_id, value)
            {
                return true;
            }
            if call.pending_effect_request == Some(request_id) {
                call.result = value.clone();
                call.pending_effect_request = None;
                return true;
            }
            false
        }
        if let Some(call) = self.suspended_call.as_deref_mut()
            && inject(call, request_id, &value)
        {
            return Ok(());
        }
        if self.pending_effect_request == Some(request_id)
            || (self.pending_effect_request.is_none() && self.pending_expression.is_some())
        {
            self.result = value;
            self.pending_effect_request = None;
            return Ok(());
        }
        Err(Fail::new(
            "vm.effect_result_request_unknown",
            format!("no suspended activation awaits request {request_id}"),
        ))
    }

    pub fn take_pending_expression(&mut self) -> Option<PendingExpression> {
        self.pending_expression.take()
    }

    pub fn restore_pending_expression(&mut self, pending: Option<PendingExpression>) {
        self.pending_expression = pending;
    }

    pub fn set_next_effect_request_id(&mut self, request_id: u64) {
        self.next_effect_request_id = request_id;
    }

    pub fn next_effect_request_id(&self) -> u64 {
        self.next_effect_request_id
    }

    /// Drain effects in script execution order.
    pub fn take_effects(&mut self) -> Vec<ScriptEffect> {
        std::mem::take(&mut self.effects)
    }

    /// Initialize the native portion of an `EX_Iterator`. The surrounding
    /// opcode consumes the end address and performs the first bind.
    pub fn begin_iterator(
        &mut self,
        args: &CallArgs,
        output_index: usize,
        values: Vec<ObjectId>,
    ) -> VmResult<()> {
        if self.pending_iterator.is_some() {
            return Err(Fail::new(
                "native.iterator_reentrant",
                "iterator native initialized two cursors before EX_Iterator consumed one",
            ));
        }
        let output = args.lvalue(output_index).cloned().ok_or_else(|| {
            Fail::new(
                "native.iterator_output_lvalue_missing",
                "iterator output must be a writable lvalue",
            )
        })?;
        self.pending_iterator = Some(PendingIterator {
            values: values
                .into_iter()
                .map(|actor| IteratorItem {
                    actor,
                    hit_location: None,
                    hit_normal: None,
                })
                .collect(),
            output,
            hit_location: None,
            hit_normal: None,
            clear_actor_on_exhaust: false,
        });
        Ok(())
    }

    /// Capture all `TraceActors` out lvalues while its argument bytecode is
    /// live. The engine result is converted into a pending iterator when the
    /// suspended EX_Iterator resumes.
    pub fn begin_deferred_trace_iterator(
        &mut self,
        args: &CallArgs,
        actor_index: usize,
        hit_location_index: usize,
        hit_normal_index: usize,
    ) -> VmResult<()> {
        let out = |index: usize| {
            args.lvalue(index).cloned().ok_or_else(|| {
                Fail::new(
                    "native.iterator_output_lvalue_missing",
                    format!("TraceActors argument {index} must be a writable lvalue"),
                )
            })
        };
        self.pending_trace_iterator = Some((
            out(actor_index)?,
            out(hit_location_index)?,
            out(hit_normal_index)?,
        ));
        Ok(())
    }

    /// Capture `AActor::Trace` out lvalues while its argument bytecode is
    /// live. The engine result is bound before the native's object return is
    /// exposed to the enclosing expression.
    pub fn begin_deferred_trace(
        &mut self,
        args: &CallArgs,
        hit_location_index: usize,
        hit_normal_index: usize,
    ) -> VmResult<()> {
        let out = |index: usize| {
            args.lvalue(index).cloned().ok_or_else(|| {
                Fail::new(
                    "native.trace_output_lvalue_missing",
                    format!("Trace argument {index} must be a writable lvalue"),
                )
            })
        };
        self.defer_expression(PendingExpression::BindTrace {
            hit_location: out(hit_location_index)?,
            hit_normal: out(hit_normal_index)?,
        });
        Ok(())
    }

    fn bind_iterator_item(
        &mut self,
        item: &IteratorItem,
        output: LValue,
        hit_location: Option<LValue>,
        hit_normal: Option<LValue>,
    ) -> VmResult<()> {
        self.assign_lvalue(output, PropValue::Object(Some(item.actor.0 as i32)))?;
        if let (Some(target), Some(value)) = (hit_location, item.hit_location.clone()) {
            self.assign_lvalue(target, value)?;
        }
        if let (Some(target), Some(value)) = (hit_normal, item.hit_normal.clone()) {
            self.assign_lvalue(target, value)?;
        }
        Ok(())
    }

    /// Consume the compiler's iterator-end operand and activate the pending
    /// native cursor. Result-bearing iterators call this while resuming the
    /// deferred outer EX_Iterator expression.
    fn activate_iterator_cursor(&mut self) -> VmResult<()> {
        let runtime_end = usize::from(self.u16_at()?);
        let end_offset = self.serialized_offset(runtime_end)?;
        let end_pc = end_offset.checked_add(1).ok_or_else(|| {
            Fail::new("vm.iterator_end_invalid", "iterator end offset overflowed")
        })?;
        if end_pc > self.code.len()
            || self.code.get(end_offset).copied() != Some(USToken::IteratorPop as u8)
        {
            return Err(Fail::new(
                "vm.iterator_end_invalid",
                format!(
                    "EX_Iterator runtime end {runtime_end} maps to serialized byte {end_offset} on {:?}, expected EX_IteratorPop",
                    self.code.get(end_offset).copied()
                ),
            ));
        }
        let pending = self.pending_iterator.take().ok_or_else(|| {
            Fail::new(
                "native.iterator_uninitialized",
                "iterator expression did not initialize a native iterator cursor",
            )
        })?;
        let body_pc = self.pc;
        let Some(first) = pending.values.first().cloned() else {
            if pending.clear_actor_on_exhaust {
                self.assign_lvalue(pending.output, PropValue::Object(None))?;
            }
            self.pc = end_pc;
            return Ok(());
        };
        self.bind_iterator_item(
            &first,
            pending.output.clone(),
            pending.hit_location.clone(),
            pending.hit_normal.clone(),
        )?;
        self.iterators.push(IteratorCursor {
            values: pending.values,
            index: 0,
            output: pending.output,
            hit_location: pending.hit_location,
            hit_normal: pending.hit_normal,
            clear_actor_on_exhaust: pending.clear_actor_on_exhaust,
            body_pc,
            end_pc,
        });
        Ok(())
    }

    /// Preserve active iterator cursors with a parked top-level frame.
    pub fn take_iterators(&mut self) -> Vec<IteratorCursor> {
        std::mem::take(&mut self.iterators)
    }

    /// Restore iterator cursors before a parked frame continues.
    pub fn restore_iterators(&mut self, iterators: Vec<IteratorCursor>) {
        self.iterators = iterators;
    }

    /// Embedder hook for the active level's actor list. The values are copied
    /// at frame start so iterator order remains stable while effects drain.
    pub fn set_actor_scope(&mut self, actors: &[ObjectId]) {
        self.actor_scope = Some(actors.to_vec());
    }

    /// Active actor ids supplied by the embedding engine, when available.
    pub fn actor_scope(&self) -> Option<&[ObjectId]> {
        self.actor_scope.as_deref()
    }

    /// Park this frame (first suspension wins — a later latent in the
    /// same statement must not overwrite the earlier wake).
    pub fn suspend_for(&mut self, kind: Suspend) {
        if self.suspend.is_none() {
            self.suspend = Some(kind);
        }
    }

    /// Queue one scheduler side effect. A runaway hot loop must not grow
    /// the queue without bound; exceeding the budget is a loud failure.
    pub fn request(&mut self, req: LatentRequest) -> VmResult<()> {
        if self.latent_requests.len() >= LATENT_REQUEST_BUDGET {
            return Err(Fail::new(
                "vm.latent_budget_exceeded",
                format!("latent-request queue exceeded {LATENT_REQUEST_BUDGET} entries"),
            ));
        }
        self.latent_requests.push(req);
        Ok(())
    }

    /// Name-pool index of the state whose code this frame executes.
    pub fn active_state(&self) -> Option<u32> {
        self.active_state
    }

    /// Embedder hook: declare a state name without a state-object scope.
    /// This supports state-aware natives but deliberately cannot affect
    /// virtual lookup.
    pub fn set_active_state(&mut self, name_index: Option<u32>) {
        self.active_state = name_index;
        self.active_state_object = None;
        self.active_state_owner = None;
    }

    /// Declare the concrete state frame that owns this bytecode. C++ virtual
    /// dispatch searches this scope before the class chain; global dispatch
    /// does not.
    pub fn set_active_state_context(&mut self, name_index: Option<u32>, state: Option<ObjectId>) {
        self.active_state = name_index;
        self.active_state_object = state;
        self.active_state_owner = self.self_id;
    }

    /// Current program cursor — where [`Frame::run`] stopped (or would).
    pub fn pc(&self) -> usize {
        self.pc
    }

    /// Convert an absolute in-memory `UStruct::Script` coordinate embedded
    /// in this frame's bytecode to its serialized byte position.
    pub fn serialized_pc_for_runtime(&self, runtime_pc: usize) -> VmResult<usize> {
        self.serialized_offset(runtime_pc)
    }

    /// Reposition the program cursor for a resumed run. Only the embedder
    /// (which saved it from a prior run on the SAME code stream) calls
    /// this, so out-of-range cursors stay a debug assertion.
    pub fn set_pc(&mut self, pc: usize) {
        debug_assert!(pc <= self.code.len(), "resume pc outside code stream");
        self.pc = pc;
    }

    /// Take the instance/default-variable writes queued by `EX_Let` during
    /// the run. The embedding harness applies them via
    /// `ObjectArena::get_mut`; until it does they are visible nowhere else.
    pub fn take_instance_writes(&mut self) -> Vec<InstanceWrite> {
        std::mem::take(&mut self.pending_writes)
    }

    fn nul_terminated_string(&mut self) -> VmResult<String> {
        let start = self.pc;
        let mut end = self.pc;
        while let Some(&b) = self.code.get(end) {
            if b == 0 {
                break;
            }
            end += 1;
        }
        self.pc = end + 1;
        Ok(String::from_utf8_lossy(&self.code[start..end]).into_owned())
    }
}

/// Slots implemented with `P_GET_*_REF` operands. `AActor::Trace` has two
/// vector outputs; the remaining ranges take their first operand by reference.
fn native_arg_is_lvalue(slot: u16, argument: usize) -> bool {
    (slot == 0x0115 && argument < 2) || (argument == 0 && native_first_arg_is_lvalue(slot))
}

/// Slots implemented in `UnCorSc.cpp` with a `P_GET_*_REF` first operand.
/// The ranges are the engine's `IMPLEMENT_FUNCTION` ids, not inferred from
/// operator spelling: byte 133..140, int 159..166, float 182..185, vector
/// 221..224/297, and rotator 290..291/318..319. Slot 225 (`VSize`) takes
/// `P_GET_VECTOR`, not `P_GET_VECTOR_REF`; treating it as writable makes a
/// valid nested vector expression (notably BaseCam's opcode `0xd8` subtract)
/// fail before the native body.
fn native_first_arg_is_lvalue(slot: u16) -> bool {
    matches!(
        slot,
        0x0085..=0x008c
            | 0x009f..=0x00a6
            | 0x00b6..=0x00b9
            | 0x00dd..=0x00e0
            | 0x0122..=0x0123
            | 0x0129
            | 0x013e..=0x013f
    )
}

fn numeric_index(value: &PropValue) -> VmResult<usize> {
    let raw = match value {
        PropValue::Byte(value) => i32::from(*value),
        PropValue::Int(value) => *value,
        _ => {
            return Err(Fail::new(
                "vm.lvalue_index_type",
                format!("array lvalue index is not integral: {value:?}"),
            ));
        }
    };
    usize::try_from(raw).map_err(|_| {
        Fail::new(
            "vm.array_out_of_bounds",
            format!("array lvalue index {raw} is negative"),
        )
    })
}

fn array_item(value: &PropValue, index: usize) -> VmResult<&PropValue> {
    let items = match value {
        PropValue::Array(items) | PropValue::FixedArray(items) => items,
        other => {
            return Err(Fail::new(
                "vm.lvalue_unsupported",
                format!("array-element lvalue has non-array base {other:?}"),
            ));
        }
    };
    items.get(index).ok_or_else(|| {
        Fail::new(
            "vm.array_out_of_bounds",
            format!("array lvalue index {index} outside 0..{}", items.len()),
        )
    })
}

fn array_item_mut(value: &mut PropValue, index: usize) -> VmResult<&mut PropValue> {
    let items = match value {
        PropValue::Array(items) | PropValue::FixedArray(items) => items,
        other => {
            return Err(Fail::new(
                "vm.lvalue_unsupported",
                format!("array-element lvalue has non-array base {other:?}"),
            ));
        }
    };
    let len = items.len();
    items.get_mut(index).ok_or_else(|| {
        Fail::new(
            "vm.array_out_of_bounds",
            format!("array lvalue index {index} outside 0..{len}"),
        )
    })
}

/// Fork-undocumented opcode gaps (`UnStack.h` defines nothing here):
/// hitting one means shipped bytecode uses an encoding this VM does not
/// model yet — loud, but distinguishable from genuine corruption.
fn is_undocumented_gap(opcode: u8) -> bool {
    matches!(opcode, 0x03 | 0x35 | 0x5B..=0x5F)
}

/// Unknown-opcode failure with a deferral-specific reason when the byte
/// sits in a fork-undocumented gap.
fn unknown_token(opcode: u8, at: usize) -> Fail {
    let code = if is_undocumented_gap(opcode) {
        "vm.unknown_token_deferrable"
    } else {
        "vm.unknown_token"
    };
    Fail::new(code, format!("opcode {opcode:#04x} at {at} has no token"))
}

fn truncated() -> Fail {
    Fail::new("vm.code_truncated", "operand runs past end of code")
}

fn instance_value(frame: &Frame<'_>, property: ObjectId, name: u32) -> VmResult<PropValue> {
    let Some(self_id) = frame.self_id else {
        return frame.arena.default_for_property(property);
    };
    if let Some(value) = frame.overlay_value(self_id, name) {
        return property_value(frame.arena, property, Some(&value));
    }
    if let Some(value) = arena_properties(frame.arena, self_id).and_then(|store| store.get(name)) {
        return property_value(frame.arena, property, Some(value));
    }
    let defaults = frame
        .arena
        .get(self_id)
        .ok()
        .and_then(|object| object.class_id)
        .and_then(|class_id| frame.arena.class_chain(class_id).ok())
        .into_iter()
        .flatten()
        .find_map(|class_id| {
            let default_object = match &frame.arena.get(class_id).ok()?.data {
                ObjectData::Class(data) => data.default_object,
                _ => None,
            }?;
            frame.overlay_value(default_object, name).or_else(|| {
                arena_properties(frame.arena, default_object)
                    .and_then(|store| store.get(name))
                    .cloned()
            })
        });
    property_value(frame.arena, property, defaults.as_ref())
}

/// `EX_DefaultVariable` reads the executing object's class-default storage,
/// not the instance bag used by `EX_InstanceVariable`.
fn default_value(frame: &Frame<'_>, property: ObjectId, name: u32) -> VmResult<PropValue> {
    let defaults = frame
        .self_id
        .and_then(|id| frame.arena.get(id).ok()?.class_id)
        .and_then(|class_id| frame.arena.class_chain(class_id).ok())
        .into_iter()
        .flatten()
        .find_map(|class_id| {
            let default_object = match &frame.arena.get(class_id).ok()?.data {
                ObjectData::Class(data) => data.default_object,
                _ => None,
            }?;
            frame.overlay_value(default_object, name).or_else(|| {
                arena_properties(frame.arena, default_object)
                    .and_then(|store| store.get(name))
                    .cloned()
            })
        });
    property_value(frame.arena, property, defaults.as_ref())
}

/// UE omits the array-index tag for element zero, so older instance bags can
/// legitimately expose that serialized fragment as a scalar even though the
/// UProperty template remains the authoritative `ArrayDim`.
fn property_value(
    arena: &ObjectArena,
    property: ObjectId,
    stored: Option<&PropValue>,
) -> VmResult<PropValue> {
    let data = match &arena.get(property)?.data {
        ObjectData::Property(data) => data,
        _ => return arena.default_for_property(property),
    };
    if data.array_dim <= 1 {
        return stored
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| arena.default_for_property(property));
    }
    match stored {
        Some(value @ (PropValue::Array(_) | PropValue::FixedArray(_))) => Ok(value.clone()),
        Some(value) => {
            let mut array = match arena.default_for_property(property)? {
                PropValue::FixedArray(items) => items,
                _ => unreachable!("ArrayDim > 1 always materializes FixedArray"),
            };
            if let Some(first) = array.first_mut() {
                *first = value.clone();
            }
            Ok(PropValue::FixedArray(array))
        }
        None => arena.default_for_property(property),
    }
}

fn arena_properties(arena: &ObjectArena, id: ObjectId) -> Option<&PropStore> {
    arena.get(id).ok()?.properties()
}

/// `execDynamicCast`/`execMetaCast` (`UnCorSc.cpp`) return the original
/// object only when the runtime type test succeeds; a failed cast is the
/// source-level `None`, not a VM error.
fn cast(
    arena: &ObjectArena,
    token: USToken,
    class: ObjectId,
    value: &PropValue,
) -> VmResult<PropValue> {
    let PropValue::Object(Some(raw)) = value else {
        return Ok(PropValue::Object(None));
    };
    let Ok(raw) = u32::try_from(*raw) else {
        return Ok(PropValue::Object(None));
    };
    let object = ObjectId(raw);
    let matches = match token {
        USToken::DynamicCast => arena.get(object)?.class_id.is_some_and(|runtime_class| {
            arena.class_is_a(runtime_class, class).unwrap_or(false)
                || arena.class_chain(runtime_class).ok().is_some_and(|chain| {
                    let wanted = arena
                        .get(class)
                        .ok()
                        .and_then(|object| arena.names.text(object.name_index));
                    wanted.is_some_and(|wanted| {
                        chain.into_iter().any(|candidate| {
                            arena
                                .get(candidate)
                                .ok()
                                .and_then(|object| arena.names.text(object.name_index))
                                .is_some_and(|name| name.eq_ignore_ascii_case(wanted))
                        })
                    })
                })
        }),
        USToken::MetaCast => {
            matches!(arena.get(object)?.data, ObjectData::Class(_))
                && arena.class_is_a(object, class)?
        }
        _ => false,
    };
    Ok(if matches {
        value.clone()
    } else {
        PropValue::Object(None)
    })
}

/// Value conversions implemented by `UnCorSc.cpp`.
fn convert(arena: &ObjectArena, token: USToken, value: &PropValue) -> VmResult<PropValue> {
    use PropValue::*;
    let int_of = |v: &PropValue| -> i32 {
        match v {
            Byte(b) => i32::from(*b),
            Int(i) => *i,
            Bool(b) => i32::from(*b),
            Float(f) => *f as i32,
            Str(text) => text.trim().parse::<i32>().unwrap_or(0),
            other => i32::from(other.truthy()),
        }
    };
    let float_of = |v: &PropValue| -> f32 {
        match v {
            Byte(b) => f32::from(*b),
            Int(i) => *i as f32,
            Float(f) => *f,
            Str(text) => text.trim().parse::<f32>().unwrap_or(0.0),
            other => f32::from(other.truthy()),
        }
    };
    Ok(match token {
        USToken::ByteToInt | USToken::StringToInt | USToken::FloatToInt | USToken::BoolToInt => {
            Int(int_of(value))
        }
        USToken::IntToFloat | USToken::StringToFloat => Float(float_of(value)),
        USToken::ByteToFloat => Float(f32::from(int_of(value) as u8)),
        USToken::IntToByte | USToken::StringToByte | USToken::FloatToByte | USToken::BoolToByte => {
            Byte(int_of(value) as u8)
        }
        USToken::ByteToBool
        | USToken::IntToBool
        | USToken::BoolToFloat
        | USToken::FloatToBool
        | USToken::StringToBool
        | USToken::ObjectToBool
        | USToken::NameToBool
        | USToken::VectorToBool
        | USToken::RotatorToBool => Bool(value.truthy()),
        USToken::IntToString => Str(match value {
            Int(value) => value.to_string(),
            other => int_of(other).to_string(),
        }),
        USToken::ByteToString => Str(match value {
            Byte(value) => value.to_string(),
            other => (int_of(other) as u8).to_string(),
        }),
        USToken::BoolToString => Str(if value.truthy() { "True" } else { "False" }.to_string()),
        USToken::FloatToString => Str(match value {
            Float(value) => value.to_string(),
            other => float_of(other).to_string(),
        }),
        USToken::ObjectToString => Str(match value {
            Object(Some(raw)) if *raw >= 0 => arena
                .path_of(ObjectId(*raw as u32))
                .unwrap_or_else(|_| "None".to_string()),
            _ => "None".to_string(),
        }),
        USToken::NameToString => Str(match value {
            Name(name) => arena.names.text(name.index).unwrap_or("None").to_string(),
            _ => "None".to_string(),
        }),
        USToken::StringToName => Name(string_to_name(arena, value)?),
        USToken::VectorToString => Str(vector_to_string(arena, value)?),
        USToken::RotatorToVector => rotator_to_vector(arena, value)?,
        USToken::VectorToRotator => vector_to_rotator(arena, value)?,
        _ => {
            return Err(Fail::new(
                "vm.conversion_unsupported",
                format!("{token:?}: composite conversion is deferred"),
            ));
        }
    })
}

fn string_to_name(arena: &ObjectArena, value: &PropValue) -> VmResult<Name> {
    let PropValue::Str(text) = value else {
        return Err(Fail::new(
            "vm.conversion_unsupported",
            format!("StringToName requires a String, received {value:?}"),
        ));
    };
    let (base, number) = crate::name::split_name_number(text);
    if base.is_empty() || base.eq_ignore_ascii_case("None") {
        return Ok(Name::none());
    }
    // UnCorSc.cpp uses FName(*Str, FNAME_Find): UnName.cpp maps a miss to
    // NAME_None rather than adding arbitrary runtime strings to the pool.
    let index = arena
        .names
        .find_index(base)
        .unwrap_or(crate::name::NAME_NONE);
    Ok(if index == crate::name::NAME_NONE {
        Name::none()
    } else {
        Name { index, number }
    })
}

fn vector_to_string(arena: &ObjectArena, value: &PropValue) -> VmResult<String> {
    let PropValue::Struct {
        struct_name,
        fields,
    } = value
    else {
        return Err(Fail::new(
            "vm.conversion_unsupported",
            format!("VectorToString requires a Vector, received {value:?}"),
        ));
    };
    if !arena
        .names
        .text(*struct_name)
        .is_some_and(|name| name.eq_ignore_ascii_case("Vector"))
    {
        return Err(Fail::new(
            "vm.conversion_unsupported",
            format!("VectorToString requires a Vector, received {value:?}"),
        ));
    }
    let component = |wanted: &str| -> VmResult<f32> {
        fields
            .iter()
            .find(|(name, _)| {
                arena
                    .names
                    .text(*name)
                    .is_some_and(|text| text.eq_ignore_ascii_case(wanted))
            })
            .and_then(|(_, value)| match value {
                PropValue::Float(value) => Some(*value),
                _ => None,
            })
            .ok_or_else(|| {
                Fail::new(
                    "vm.conversion_unsupported",
                    format!("VectorToString requires a numeric {wanted} component"),
                )
            })
    };
    let x = component("X")?;
    let y = component("Y")?;
    let z = component("Z")?;
    // UnCorSc.cpp execVectorToString uses FString::Printf("%f,%f,%f"):
    // C printf's default `%f` precision is six digits after the decimal.
    Ok(format!("{x:.6},{y:.6},{z:.6}"))
}

fn rotator_to_vector(arena: &ObjectArena, value: &PropValue) -> VmResult<PropValue> {
    let PropValue::Struct { fields, .. } = value else {
        return Err(Fail::new(
            "vm.conversion_unsupported",
            format!("RotatorToVector requires a Rotator, received {value:?}"),
        ));
    };
    let component = |wanted: &str| {
        fields
            .iter()
            .find(|(name, _)| {
                arena
                    .names
                    .text(*name)
                    .is_some_and(|text| text.eq_ignore_ascii_case(wanted))
            })
            .and_then(|(_, value)| match value {
                PropValue::Int(value) => Some(*value),
                _ => None,
            })
            .unwrap_or(0)
    };
    // UnCorSc.cpp execRotatorToVector delegates to FRotator::Vector
    // (UnMath.h:2658-2661): roll is irrelevant to the forward X axis.
    let pitch = component("Pitch") as f32 * std::f32::consts::TAU / 65536.0;
    let yaw = component("Yaw") as f32 * std::f32::consts::TAU / 65536.0;
    let (sin_pitch, cos_pitch) = pitch.sin_cos();
    let (sin_yaw, cos_yaw) = yaw.sin_cos();
    let vector = arena
        .find_by_path("Core.Vector")
        .or_else(|| {
            arena
                .objects_iter()
                .find(|(_, object)| {
                    matches!(object.data, ObjectData::ScriptStruct(_))
                        && arena
                            .names
                            .text(object.name_index)
                            .is_some_and(|name| name.eq_ignore_ascii_case("Vector"))
                })
                .map(|(id, _)| id)
        })
        .ok_or_else(|| {
            Fail::new(
                "vm.conversion_unsupported",
                "RotatorToVector requires the Core Vector script struct",
            )
        })?;
    let struct_name = arena.get(vector)?.name_index;
    let values = [
        ("X", cos_pitch * cos_yaw),
        ("Y", cos_pitch * sin_yaw),
        ("Z", sin_pitch),
    ];
    let mut out = Vec::with_capacity(3);
    for (wanted, value) in values {
        let name = arena
            .children_of(vector)
            .find_map(|(_, child)| {
                arena
                    .names
                    .text(child.name_index)
                    .is_some_and(|text| text.eq_ignore_ascii_case(wanted))
                    .then_some(child.name_index)
            })
            .ok_or_else(|| {
                Fail::new(
                    "vm.conversion_unsupported",
                    format!("Core.Vector has no {wanted} property"),
                )
            })?;
        out.push((name, PropValue::Float(value)));
    }
    Ok(PropValue::Struct {
        struct_name,
        fields: out,
    })
}

fn vector_to_rotator(arena: &ObjectArena, value: &PropValue) -> VmResult<PropValue> {
    let PropValue::Struct { fields, .. } = value else {
        return Err(Fail::new(
            "vm.conversion_unsupported",
            format!("VectorToRotator requires a Vector, received {value:?}"),
        ));
    };
    let component = |wanted: &str| {
        fields
            .iter()
            .find(|(name, _)| {
                arena
                    .names
                    .text(*name)
                    .is_some_and(|text| text.eq_ignore_ascii_case(wanted))
            })
            .and_then(|(_, value)| match value {
                PropValue::Float(value) => Some(*value),
                PropValue::Int(value) => Some(*value as f32),
                _ => None,
            })
            .unwrap_or(0.0)
    };
    let x = component("X");
    let y = component("Y");
    let z = component("Z");
    // FVector::Rotation (`UnMath.cpp`): multiply by MAXWORD/(2*PI), where
    // MAXWORD is 65535, then use C++'s truncating float-to-INT conversion.
    // Yaw is atan2(Y,X), pitch is atan2(Z, horizontal magnitude), and a
    // direction vector has no roll.
    let units = 65535.0 / (2.0 * std::f32::consts::PI);
    let pitch = (z.atan2(x.hypot(y)) * units) as i32;
    let yaw = (y.atan2(x) * units) as i32;
    let rotator = arena
        .find_by_path("Core.Rotator")
        .or_else(|| {
            arena
                .objects_iter()
                .find(|(_, object)| {
                    matches!(object.data, ObjectData::ScriptStruct(_))
                        && arena
                            .names
                            .text(object.name_index)
                            .is_some_and(|name| name.eq_ignore_ascii_case("Rotator"))
                })
                .map(|(id, _)| id)
        })
        .ok_or_else(|| {
            Fail::new(
                "vm.conversion_unsupported",
                "VectorToRotator requires the Core Rotator script struct",
            )
        })?;
    let struct_name = arena.get(rotator)?.name_index;
    let values = [("Pitch", pitch), ("Yaw", yaw), ("Roll", 0)];
    let mut out = Vec::with_capacity(3);
    for (wanted, value) in values {
        let name = arena
            .children_of(rotator)
            .find_map(|(_, child)| {
                arena
                    .names
                    .text(child.name_index)
                    .is_some_and(|text| text.eq_ignore_ascii_case(wanted))
                    .then_some(child.name_index)
            })
            .ok_or_else(|| {
                Fail::new(
                    "vm.conversion_unsupported",
                    format!("Core.Rotator has no {wanted} property"),
                )
            })?;
        out.push((name, PropValue::Int(value)));
    }
    Ok(PropValue::Struct {
        struct_name,
        fields: out,
    })
}
// ---- State entry-point resolution -------------------------------------------

/// Decode one compact index at `pos` without mutating anything; returns
/// the value and the position past it. Mirrors [`Frame::name_ci`]'s
/// decoding exactly.
fn compact_index_at(code: &[u8], pos: usize) -> Option<(i32, usize)> {
    let first = *code.get(pos)? as u32;
    let mut next = pos + 1;

    let negative = first & 0x80 != 0;
    let mut magnitude = (first & 0x3F) as i64;
    let mut shift = 6;
    if first & 0x40 != 0 {
        loop {
            let byte = *code.get(next)?;
            next += 1;
            magnitude |= i64::from(byte & 0x7F) << shift;
            shift += 7;
            if byte & 0x80 == 0 || shift >= 35 {
                break;
            }
        }
    }
    Some((if negative { -magnitude } else { magnitude } as i32, next))
}

fn i32_at_slice(code: &[u8], pos: usize) -> Option<i32> {
    code.get(pos..pos + 4)?
        .try_into()
        .ok()
        .map(i32::from_le_bytes)
}

/// Resolve the `Begin:` entry program counter of one state. State code has
/// no valid recovery path: a missing or malformed label table would make a
/// resumed frame execute data as bytecode, so every such condition is a
/// reason-coded `vm.state_*` failure rather than an offset-zero fallback.
pub fn state_entry_pc(
    arena: &ObjectArena,
    resolver: &BytecodeResolver,
    code: &[u8],
    label_table_offset: usize,
) -> VmResult<usize> {
    if code.is_empty() {
        return Err(Fail::new(
            "vm.state_code_empty",
            "state has no executable bytecode",
        ));
    }
    let label_table_offset = if label_table_offset == 0 {
        0
    } else {
        serialized_offset_for_runtime(arena, resolver, code, label_table_offset)?
    };
    if label_table_offset == 0 || label_table_offset >= code.len() {
        return Err(Fail::new(
            "vm.state_label_table_offset",
            format!(
                "label table offset {label_table_offset} outside {}-byte state code",
                code.len()
            ),
        ));
    }
    let mut pos = label_table_offset;
    while pos < code.len() {
        let Some((raw_name, after_name)) = compact_index_at(code, pos) else {
            return Err(Fail::new(
                "vm.state_label_table_malformed",
                "state label table has a truncated name index",
            ));
        };
        let name = resolver.resolve_name(raw_name)?;
        if name == 0 {
            return Err(Fail::new(
                "vm.state_begin_missing",
                "state label table carries no Begin label",
            ));
        }
        let Some(offset) = i32_at_slice(code, after_name) else {
            return Err(Fail::new(
                "vm.state_label_table_malformed",
                "state label table has a truncated code offset",
            ));
        };
        let text = arena.names.text(name).ok_or_else(|| {
            Fail::new(
                "vm.state_label_name_invalid",
                format!("state label table references missing global name index {name}"),
            )
        })?;
        if crate::name::fold_key(text) == "begin" {
            if offset >= 0 {
                let serialized =
                    serialized_offset_for_runtime(arena, resolver, code, offset as usize)?;
                if serialized < label_table_offset {
                    return Ok(serialized);
                }
            }
            return Err(Fail::new(
                "vm.state_entry_out_of_range",
                format!(
                    "Begin label runtime offset {offset} outside executable state code ending at serialized byte {label_table_offset}"
                ),
            ));
        }
        pos = after_name + 4;
    }
    Err(Fail::new(
        "vm.state_label_table_malformed",
        "state label table has no NAME_None terminator",
    ))
}
/// Case-folded comparison key for script names (`GetStateName`,
/// `IsInState`, state resolution). Re-exported so embedding crates share
/// the interpreter's exact folding.
pub fn fold_key(text: &str) -> String {
    crate::name::fold_key(text)
}

// ---- Structural bytecode census ---------------------------------------------

/// Aggregate statistics for one walked code stream, collected by
/// [`census_stream`]: a linear, operand-correct walk that mirrors
/// [`Frame`]'s operand consumption byte-for-byte while never diverting
/// control flow — branch bodies and otherwise-unreachable regions are
/// valid bytecode and must stay covered.
#[derive(Debug, Default, Clone)]
pub struct StreamStats {
    /// Total opcodes decoded (nested expression opcodes included).
    pub ops: u64,
    /// Opcode histogram keyed by token name; numbered natives aggregate
    /// under `"NumberedNative"`, the 0x61..=0x6F nibble group under
    /// `"ExtendedNativeHi"` (their slots land in `native_slots`).
    pub opcode_hist: std::collections::BTreeMap<String, u64>,
    /// NAME-based virtual/global call targets. Final calls are recorded by
    /// their raw object reference because this structural walker intentionally
    /// has no package resolver.
    pub named_calls: std::collections::BTreeMap<String, u64>,
    /// Numbered native slots: bare 0x70..=0xFF map to themselves,
    /// 0x61..=0x6F span `(op - 0x60) * 0x100 + low`, and a literal
    /// `EX_ExtendedNative` carries its low byte directly — mirroring
    /// [`Frame`] exactly.
    pub native_slots: std::collections::BTreeMap<u16, u64>,
    pub goto_label: u64,
    /// Named calls to `GotoState`.
    pub goto_state_calls: u64,
    pub label_tables: u64,
    /// Non-terminal entries across all label tables.
    pub label_entries: u64,
    pub vector_consts: u64,
    pub rotation_consts: u64,
    pub array_element: u64,
    pub dyn_array_element: u64,
    pub iterator: u64,
    pub new_expr: u64,
    pub context: u64,
    pub struct_member: u64,
    /// `Let`/`LetBool` through an instance/default-variable lvalue.
    pub instance_writes: u64,
}

impl StreamStats {
    /// Fold another stream's counters into this one.
    pub fn merge(&mut self, other: &StreamStats) {
        self.ops += other.ops;
        for (key, count) in &other.opcode_hist {
            *self.opcode_hist.entry(key.clone()).or_insert(0) += count;
        }
        for (key, count) in &other.named_calls {
            *self.named_calls.entry(key.clone()).or_insert(0) += count;
        }
        for (slot, count) in &other.native_slots {
            *self.native_slots.entry(*slot).or_insert(0) += count;
        }
        self.goto_label += other.goto_label;
        self.goto_state_calls += other.goto_state_calls;
        self.label_tables += other.label_tables;
        self.label_entries += other.label_entries;
        self.vector_consts += other.vector_consts;
        self.rotation_consts += other.rotation_consts;
        self.array_element += other.array_element;
        self.dyn_array_element += other.dyn_array_element;
        self.iterator += other.iterator;
        self.new_expr += other.new_expr;
        self.context += other.context;
        self.struct_member += other.struct_member;
        self.instance_writes += other.instance_writes;
    }

    /// True when `name` (case-folded) is a known latent-function marker.
    pub fn is_latent(name: &str) -> bool {
        matches!(
            crate::name::fold_key(name).as_str(),
            "sleep"
                | "finishanim"
                | "moveto"
                | "movetoward"
                | "turnto"
                | "waitfortimer"
                | "settimer"
        )
    }

    /// Named calls whose target is a known latent function.
    pub fn latent_total(&self) -> u64 {
        self.named_calls
            .iter()
            .filter(|(name, _)| Self::is_latent(name))
            .map(|(_, count)| count)
            .sum()
    }
}

/// Why a structural walk stopped before consuming its stream. Data, not
/// a crash: callers record it and move to the next stream.
#[derive(Debug, Clone)]
pub struct CensusAbort {
    pub reason: String,
    pub offset: usize,
}

/// Hard cap on decoded opcodes per stream — pure paranoia against a
/// desync turning a walk into an unbounded loop; real streams finish in
/// `O(code.len())`.
const CENSUS_STEP_BUDGET: u64 = 1 << 22;

/// Walk one token stream structurally: every opcode is classified and its
/// operands consumed exactly as [`Frame] would consume them, but control
/// flow falls straight through (`Jump`/`JumpIfNot` targets are data, not
/// destinations; `Stop`/`Return` end nothing). Decode failures abort with
/// the offending offset instead of panicking.
pub fn census_stream(
    arena: &ObjectArena,
    code: &[u8],
) -> std::result::Result<StreamStats, CensusAbort> {
    let mut walk = CensusWalk::new(arena, code);
    walk.run()?;
    Ok(walk.stats)
}

/// Translate an in-memory `UStruct::Script` byte offset stored by the
/// compiler into this package archive's serialized byte position. On disk,
/// UObject and FName operands are compact indices; `SerializeExpr` expands
/// both to four-byte runtime slots (`UnClass.cpp:1431-1475`). Control-flow
/// fixups retain the expanded Script offset, so the VM must cross this map
/// before indexing the serialized code slice.
fn build_runtime_offset_map(
    arena: &ObjectArena,
    resolver: &BytecodeResolver,
    code: &[u8],
) -> (
    std::collections::BTreeMap<usize, usize>,
    Option<CensusAbort>,
) {
    let mut walk = CensusWalk::new_with_resolver(arena, resolver, code);
    let abort = walk.run().err();
    (walk.opcode_offsets, abort)
}

fn offset_from_map(
    offsets: &std::collections::BTreeMap<usize, usize>,
    abort: Option<&CensusAbort>,
    runtime_offset: usize,
) -> VmResult<usize> {
    if let Some(serialized) = offsets.get(&runtime_offset).copied() {
        return Ok(serialized);
    }
    if let Some(abort) = abort {
        return Err(Fail::new(
            "vm.offset_map_invalid",
            format!(
                "cannot map runtime script offset {runtime_offset}: {} at serialized byte {}",
                abort.reason, abort.offset
            ),
        ));
    }
    Err(Fail::new(
        "vm.offset_map_invalid",
        format!("runtime script offset {runtime_offset} is not a mapped bytecode boundary"),
    ))
}

fn relative_target_from_map(
    offsets: &std::collections::BTreeMap<usize, usize>,
    abort: Option<&CensusAbort>,
    serialized_base: usize,
    runtime_delta: usize,
) -> VmResult<usize> {
    let runtime_base = offsets
        .iter()
        .find_map(|(runtime, serialized)| (*serialized == serialized_base).then_some(*runtime))
        .ok_or_else(|| {
            Fail::new(
                "vm.offset_map_invalid",
                format!("serialized byte {serialized_base} is not an opcode boundary"),
            )
        })?;
    let runtime_target = runtime_base.checked_add(runtime_delta).ok_or_else(|| {
        Fail::new(
            "vm.offset_map_invalid",
            "relative runtime offset overflowed",
        )
    })?;
    if let Some(serialized) = offsets.get(&runtime_target).copied() {
        return Ok(serialized);
    }
    if let Some(abort) = abort {
        return Err(Fail::new(
            "vm.code_truncated",
            format!(
                "relative runtime target {runtime_target} is unavailable: {} at serialized byte {}",
                abort.reason, abort.offset
            ),
        ));
    }
    Err(Fail::new(
        "vm.code_truncated",
        format!("relative runtime target {runtime_target} is outside the code stream"),
    ))
}

fn serialized_offset_for_runtime(
    arena: &ObjectArena,
    resolver: &BytecodeResolver,
    code: &[u8],
    runtime_offset: usize,
) -> VmResult<usize> {
    let (offsets, abort) = build_runtime_offset_map(arena, resolver, code);
    offset_from_map(&offsets, abort.as_ref(), runtime_offset)
}

impl<'a> CensusWalk<'a> {
    fn new(arena: &'a ObjectArena, code: &'a [u8]) -> Self {
        Self {
            arena,
            resolver: None,
            code,
            pc: 0,
            runtime_pc: 0,
            opcode_offsets: std::collections::BTreeMap::new(),
            steps: 0,
            stats: StreamStats::default(),
        }
    }

    fn new_with_resolver(
        arena: &'a ObjectArena,
        resolver: &'a BytecodeResolver,
        code: &'a [u8],
    ) -> Self {
        let mut walk = Self::new(arena, code);
        walk.resolver = Some(resolver);
        walk
    }
}

type Step = std::result::Result<(), CensusAbort>;

struct CensusWalk<'a> {
    arena: &'a ObjectArena,
    resolver: Option<&'a BytecodeResolver>,
    code: &'a [u8],
    pc: usize,
    runtime_pc: usize,
    /// Runtime Script coordinate to serialized byte at opcode starts and
    /// expression ends used by relative Context skip fixups.
    opcode_offsets: std::collections::BTreeMap<usize, usize>,
    steps: u64,
    stats: StreamStats,
}

impl<'a> CensusWalk<'a> {
    fn run(&mut self) -> std::result::Result<(), CensusAbort> {
        while self.pc < self.code.len() {
            self.steps += 1;
            if self.steps > CENSUS_STEP_BUDGET {
                return Err(self.abort("census.step_budget"));
            }
            let opcode = self.code[self.pc];
            self.opcode_offsets.insert(self.runtime_pc, self.pc);
            self.pc += 1;
            self.runtime_pc += 1;
            self.expr_opcode(opcode)?;
        }
        self.opcode_offsets.insert(self.runtime_pc, self.pc);
        Ok(())
    }

    /// Classify one consumed opcode. Shared by the top-level loop and
    /// nested expression positions, mirroring [`Frame::expr_opcode`].
    fn expr_opcode(&mut self, opcode: u8) -> Step {
        if (0x61..=0x6F).contains(&opcode) {
            let low = self.u8_at()?;
            self.bump("ExtendedNativeHi");
            let slot = u16::from(opcode - 0x60) * 0x100 + u16::from(low);
            *self.stats.native_slots.entry(slot).or_insert(0) += 1;
            return self.eval_parms();
        }
        if (0x70..=0xFF).contains(&opcode) {
            self.bump("NumberedNative");
            *self
                .stats
                .native_slots
                .entry(u16::from(opcode))
                .or_insert(0) += 1;
            return self.eval_parms();
        }
        let Some(token) = USToken::from_opcode(opcode) else {
            return Err(CensusAbort {
                reason: if is_undocumented_gap(opcode) {
                    "census.unknown_token_deferrable".to_string()
                } else {
                    "census.unknown_token".to_string()
                },
                offset: self.pc - 1,
            });
        };
        self.bump(&format!("{token:?}"));
        self.exec(token)
    }

    fn bump(&mut self, name: &str) {
        *self.stats.opcode_hist.entry(name.to_string()).or_insert(0) += 1;
        self.stats.ops += 1;
    }

    /// Operand consumption mirror of [`Frame::exec`] — records, never
    /// evaluates, and never changes `pc` for control-flow purposes.
    fn exec(&mut self, token: USToken) -> Step {
        use USToken::*;
        match token {
            // ---- Variable loads --------------------------------------
            LocalVariable | NativeParm | InstanceVariable | DefaultVariable => {
                self.name_ci()?;
            }
            BoolVariable => {
                let sub = self.u8_at()?;
                self.expr_opcode(sub)?;
            }

            // ---- Constants -------------------------------------------
            IntConst | FloatConst => {
                self.i32_at()?;
            }
            IntConstByte | ByteConst => {
                self.u8_at()?;
            }
            IntZero | IntOne | True | False | NoObject | SelfToken => {}
            NameConst | ObjectConst => {
                self.name_ci()?;
            }
            StringConst => {
                self.nul_terminated_string()?;
            }
            UnicodeStringConst => loop {
                let lo = self.u8_at()?;
                let hi = self.u8_at()?;
                if u16::from_le_bytes([lo, hi]) == 0 {
                    break;
                }
            },
            VectorConst => {
                for _ in 0..3 {
                    self.i32_at()?;
                }
                self.stats.vector_consts += 1;
            }
            RotationConst => {
                for _ in 0..3 {
                    self.i32_at()?;
                }
                self.stats.rotation_consts += 1;
            }

            // ---- Assignment & control flow ---------------------------
            Let => {
                self.pop_lvalue()?;
                self.expr()?;
            }
            LetBool => {
                self.pop_lvalue()?;
                self.expr()?;
            }
            Jump => {
                self.u16_at()?;
            }
            JumpIfNot => {
                self.u16_at()?;
                self.expr()?;
            }
            Stop | Nothing => {}
            Return => {
                self.expr()?;
            }
            Skip => {
                self.u16_at()?;
                self.expr()?;
            }
            Assert => {
                self.u16_at()?;
                self.expr()?;
            }
            GotoLabel => {
                self.expr()?;
                self.stats.goto_label += 1;
            }
            Switch => {
                self.u8_at()?;
                self.expr()?;
            }
            Case => {
                let next = self.u16_at()?;
                if next != 0xFFFF {
                    self.expr()?;
                }
            }
            LabelTable => {
                self.stats.label_tables += 1;
                // `UState::LabelTableOffset` points immediately after the
                // EX_LabelTable token, at the first FLabelEntry.
                self.opcode_offsets.insert(self.runtime_pc, self.pc);
                loop {
                    let raw_name = self.name_ci()?;
                    self.i32_at()?;
                    let is_none = match self.resolver {
                        Some(resolver) => {
                            resolver
                                .resolve_name(raw_name)
                                .map_err(|_| self.abort("census.name_ref_unresolved"))?
                                == 0
                        }
                        None => raw_name == 0,
                    };
                    if is_none {
                        break;
                    }
                    self.stats.label_entries += 1;
                }
            }
            DebugInfo => {
                self.i32_at()?;
                self.i32_at()?;
                self.i32_at()?;
                self.nul_terminated_string()?;
            }
            LineNumber => {
                self.u16_at()?;
            }

            // ---- Calls -------------------------------------------------
            VirtualFunction | GlobalFunction => {
                let index = self.name_ci()?;
                let text = self
                    .arena
                    .names
                    .text(index as u32)
                    .unwrap_or("?")
                    .to_string();
                *self.stats.named_calls.entry(text.clone()).or_insert(0) += 1;
                if crate::name::fold_key(&text) == "gotostate" {
                    self.stats.goto_state_calls += 1;
                }
                self.eval_parms()?;
            }
            FinalFunction => {
                let raw = self.name_ci()?;
                *self
                    .stats
                    .named_calls
                    .entry(format!("<final-ref:{raw}>"))
                    .or_insert(0) += 1;
                self.eval_parms()?;
            }

            // ---- Member/array access -----------------------------------
            StructMember => {
                self.name_ci()?;
                self.expr()?;
                self.stats.struct_member += 1;
            }
            ArrayElement => {
                self.expr()?;
                self.expr()?;
                self.stats.array_element += 1;
            }
            DynArrayElement => {
                self.expr()?;
                self.expr()?;
                self.stats.dyn_array_element += 1;
            }
            DynArrayCount => {
                self.expr()?;
            }
            Context | ClassContext => {
                self.expr()?;
                self.u16_at()?;
                self.u8_at()?;
                self.expr()?;
                self.stats.context += 1;
            }
            MetaCast | DynamicCast => {
                self.name_ci()?;
                self.expr()?;
            }

            // ---- Iterators & allocation ---------------------------------
            Iterator => {
                self.expr()?;
                self.u16_at()?;
                self.stats.iterator += 1;
            }
            IteratorNext | IteratorPop => {}
            New => {
                self.expr()?;
                self.expr()?;
                self.expr()?;
                self.expr()?;
                self.stats.new_expr += 1;
            }

            // ---- Comparison & misc --------------------------------------
            StructCmpEq | StructCmpNe => {
                self.name_ci()?;
                self.expr()?;
                self.expr()?;
            }
            EatString => {
                self.expr()?;
            }
            ByteToInt | ByteToBool | ByteToFloat | IntToByte | IntToBool | IntToFloat
            | BoolToByte | BoolToInt | BoolToFloat | FloatToByte | FloatToInt | FloatToBool
            | ObjectToBool | NameToBool | StringToByte | StringToInt | StringToBool
            | StringToFloat | StringToVector | StringToRotator | VectorToBool | VectorToRotator
            | RotatorToBool | RotatorToVector | ByteToString | IntToString | BoolToString
            | FloatToString | ObjectToString | NameToString | VectorToString | RotatorToString
            | StringToName => {
                self.expr()?;
            }

            // ---- Extended natives ----------------------------------------
            ExtendedNative => {
                let low = self.u8_at()?;
                *self.stats.native_slots.entry(u16::from(low)).or_insert(0) += 1;
                self.eval_parms()?;
            }
            FirstNative => return Err(self.abort("census.first_native_unreachable")),
            EndFunctionParms => return Err(self.abort("census.unexpected_end_parms")),
        }
        Ok(())
    }

    /// `Let`/`LetBool` target. Plain variable loads carry a compact-index
    /// field name (the runtime's [`Frame::pop_lvalue_name`] case); any
    /// other lvalue is a full expression (struct-member writes and the
    /// like, which the runtime VM defers on) — consumed structurally so
    /// the walk stays aligned.
    fn pop_lvalue(&mut self) -> Step {
        let opcode = *self
            .code
            .get(self.pc)
            .ok_or_else(|| self.abort("census.code_truncated"))?;
        match USToken::from_opcode(opcode) {
            Some(
                tok @ (USToken::LocalVariable
                | USToken::InstanceVariable
                | USToken::DefaultVariable
                | USToken::NativeParm),
            ) => {
                self.opcode_offsets.insert(self.runtime_pc, self.pc);
                self.pc += 1;
                self.runtime_pc += 1;
                if matches!(tok, USToken::InstanceVariable | USToken::DefaultVariable) {
                    self.stats.instance_writes += 1;
                }
                self.name_ci()?;
                Ok(())
            }
            _ => self.expr(),
        }
    }

    /// Consume parameters up to `EX_EndFunctionParms` ([`Frame::eval_parms_and_dispatch`]).
    fn eval_parms(&mut self) -> Step {
        loop {
            let peek = *self
                .code
                .get(self.pc)
                .ok_or_else(|| self.abort("census.code_truncated"))?;
            if peek == USToken::EndFunctionParms as u8 {
                self.bump("EndFunctionParms");
                self.opcode_offsets.insert(self.runtime_pc, self.pc);
                self.pc += 1;
                self.runtime_pc += 1;
                break;
            }
            self.expr()?;
        }
        Ok(())
    }

    fn expr(&mut self) -> Step {
        let opcode = *self
            .code
            .get(self.pc)
            .ok_or_else(|| self.abort("census.code_truncated"))?;
        self.opcode_offsets.insert(self.runtime_pc, self.pc);
        self.pc += 1;
        self.runtime_pc += 1;
        self.expr_opcode(opcode)?;
        // Context null-skips are measured to the byte immediately after
        // their member expression. That is a valid runtime Script
        // coordinate even when the following serialized bytes are the
        // enclosing Context's WORD/BYTE operands rather than an opcode.
        self.opcode_offsets.insert(self.runtime_pc, self.pc);
        Ok(())
    }

    /// Compact-index decode copied verbatim from [`Frame::name_ci`].
    fn name_ci(&mut self) -> std::result::Result<i32, CensusAbort> {
        let mut pos = self.pc;
        let first = *self
            .code
            .get(pos)
            .ok_or_else(|| self.abort("census.code_truncated"))? as u32;
        pos += 1;
        let negative = first & 0x80 != 0;
        let mut magnitude = (first & 0x3F) as i64;
        let mut shift = 6;
        if first & 0x40 != 0 {
            loop {
                let byte = *self
                    .code
                    .get(pos)
                    .ok_or_else(|| self.abort("census.code_truncated"))?;
                pos += 1;
                magnitude |= i64::from(byte & 0x7F) << shift;
                shift += 7;
                if byte & 0x80 == 0 || shift >= 35 {
                    break;
                }
            }
        }

        self.pc = pos;
        self.runtime_pc += 4;
        Ok(if negative { -magnitude } else { magnitude } as i32)
    }

    fn u8_at(&mut self) -> std::result::Result<u8, CensusAbort> {
        let v = *self
            .code
            .get(self.pc)
            .ok_or_else(|| self.abort("census.code_truncated"))?;
        self.pc += 1;
        self.runtime_pc += 1;
        Ok(v)
    }

    fn u16_at(&mut self) -> std::result::Result<u16, CensusAbort> {
        let end = self.pc + 2;
        let v = u16::from_le_bytes(
            self.code
                .get(self.pc..end)
                .ok_or_else(|| self.abort("census.code_truncated"))?
                .try_into()
                .unwrap(),
        );
        self.pc = end;
        self.runtime_pc += 2;
        Ok(v)
    }

    fn i32_at(&mut self) -> std::result::Result<i32, CensusAbort> {
        let end = self.pc + 4;
        let v = i32::from_le_bytes(
            self.code
                .get(self.pc..end)
                .ok_or_else(|| self.abort("census.code_truncated"))?
                .try_into()
                .unwrap(),
        );
        self.pc = end;
        self.runtime_pc += 4;
        Ok(v)
    }

    fn nul_terminated_string(&mut self) -> std::result::Result<String, CensusAbort> {
        let start = self.pc;
        let mut end = self.pc;
        while let Some(&b) = self.code.get(end) {
            if b == 0 {
                break;
            }
            end += 1;
        }
        self.pc = end + 1;
        self.runtime_pc += end + 1 - start;
        Ok(String::from_utf8_lossy(&self.code[start..end]).into_owned())
    }

    fn abort(&self, reason: &str) -> CensusAbort {
        CensusAbort {
            reason: reason.to_string(),
            offset: self.pc.min(self.code.len()),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::natives::NativeRegistry;

    fn registry() -> NativeRegistry {
        NativeRegistry::new()
    }

    fn property_object(arena: &mut ObjectArena, name_index: u32) -> ObjectId {
        use crate::arena::{PropertyData, UObject};
        arena.alloc(UObject {
            name_index,
            data: ObjectData::Property(Box::new(PropertyData {
                links: Default::default(),
                kind: PropertyKind::Int,
                array_dim: 1,
                property_flags: 0,
                category: 0,
            })),
            ..Default::default()
        })
    }

    /// Build a package-style export resolver for synthetic bytecode. The raw
    /// values deliberately remain code operands; they resolve to real
    /// UProperty objects rather than being reinterpreted as name indices.
    fn property_resolver(arena: &mut ObjectArena, refs: &[(i32, u32)]) -> BytecodeResolver {
        let placeholder = arena.alloc(crate::arena::UObject::default());
        let mut exports = vec![
            placeholder;
            refs.iter()
                .map(|(raw, _)| usize::try_from(*raw).expect("positive raw ref"))
                .max()
                .unwrap_or(0)
        ];
        for (raw, name_index) in refs {
            assert!(*raw > 0, "zero is the package null reference");
            exports[*raw as usize - 1] = property_object(arena, *name_index);
        }
        BytecodeResolver::from_package_refs(Vec::new(), exports)
    }

    fn resolver_with_objects(
        arena: &mut ObjectArena,
        refs: &[(i32, ObjectId)],
    ) -> BytecodeResolver {
        let placeholder = arena.alloc(crate::arena::UObject::default());
        let mut exports = vec![
            placeholder;
            refs.iter()
                .map(|(raw, _)| usize::try_from(*raw).expect("positive raw ref"))
                .max()
                .unwrap_or(0)
        ];
        for (raw, object) in refs {
            assert!(*raw > 0, "zero is the package null reference");
            exports[*raw as usize - 1] = *object;
        }
        BytecodeResolver::from_package_refs(Vec::new(), exports)
    }
    fn class_object(
        arena: &mut ObjectArena,
        name: &str,
        super_class: Option<ObjectId>,
    ) -> ObjectId {
        let name_index = arena.names.intern(name);
        arena.alloc(crate::arena::UObject {
            name_index,
            data: ObjectData::Class(crate::arena::ClassData {
                super_class,
                ..Default::default()
            }),
            ..Default::default()
        })
    }

    #[test]
    fn dynamic_and_meta_casts_preserve_only_compatible_objects() {
        let mut arena = ObjectArena::new();
        let base = class_object(&mut arena, "Base", None);
        let child = class_object(&mut arena, "Child", Some(base));
        let unrelated = class_object(&mut arena, "Unrelated", None);
        let instance_name = arena.names.intern("Instance");
        let instance = arena.alloc(crate::arena::UObject {
            class_id: Some(child),
            name_index: instance_name,
            data: ObjectData::Properties(Default::default()),
            ..Default::default()
        });
        let resolver = resolver_with_objects(
            &mut arena,
            &[(1, base), (2, instance), (3, unrelated), (4, child)],
        );
        let registry = registry();
        let eval = |token: USToken, target: u8, value: u8| {
            let code = [token as u8, target, USToken::ObjectConst as u8, value];
            Frame::with_resolver(&arena, &registry, &code, None, resolver.clone())
                .run()
                .unwrap()
        };
        assert_eq!(
            eval(USToken::DynamicCast, 1, 2),
            PropValue::Object(Some(instance.0 as i32))
        );
        assert_eq!(eval(USToken::DynamicCast, 3, 2), PropValue::Object(None));
        assert_eq!(
            eval(USToken::MetaCast, 1, 4),
            PropValue::Object(Some(child.0 as i32))
        );
        assert_eq!(eval(USToken::MetaCast, 1, 2), PropValue::Object(None));
    }

    #[test]
    fn vector_to_rotator_uses_unreal_axes_and_zero_direction() {
        let mut arena = ObjectArena::new();
        let core = arena.root_for("Core");
        let rotator_name = arena.names.intern("Rotator");
        let rotator = arena.alloc(crate::arena::UObject {
            name_index: rotator_name,
            outer: Some(core),
            data: ObjectData::ScriptStruct(Default::default()),
            ..Default::default()
        });
        for component in ["Pitch", "Yaw", "Roll"] {
            let name_index = arena.names.intern(component);
            arena.alloc(crate::arena::UObject {
                name_index,
                outer: Some(rotator),
                data: ObjectData::Property(Box::new(crate::arena::PropertyData {
                    links: Default::default(),
                    kind: PropertyKind::Int,
                    array_dim: 1,
                    property_flags: 0,
                    category: 0,
                })),
                ..Default::default()
            });
        }
        let vector_name = arena.names.intern("Vector");
        let x_name = arena.names.intern("X");
        let y_name = arena.names.intern("Y");
        let z_name = arena.names.intern("Z");
        let direction = |x: f32, y: f32, z: f32| PropValue::Struct {
            struct_name: vector_name,
            fields: vec![
                (x_name, PropValue::Float(x)),
                (y_name, PropValue::Float(y)),
                (z_name, PropValue::Float(z)),
            ],
        };
        let components = |value: PropValue| {
            let PropValue::Struct { fields, .. } = value else {
                panic!("rotator struct")
            };
            ["Pitch", "Yaw", "Roll"].map(|wanted| {
                fields
                    .iter()
                    .find(|(name, _)| arena.names.text(*name).is_some_and(|name| name == wanted))
                    .and_then(|(_, value)| match value {
                        PropValue::Int(value) => Some(*value),
                        _ => None,
                    })
                    .unwrap()
            })
        };
        for (vector, expected) in [
            ((0.0, 0.0, 0.0), [0, 0, 0]),
            ((1.0, 0.0, 0.0), [0, 0, 0]),
            ((0.0, 1.0, 0.0), [0, 16383, 0]),
            ((0.0, -1.0, 0.0), [0, -16383, 0]),
            ((0.0, 0.0, 1.0), [16383, 0, 0]),
            ((-1.0, 0.0, 0.0), [0, 32767, 0]),
        ] {
            assert_eq!(
                components(
                    vector_to_rotator(&arena, &direction(vector.0, vector.1, vector.2)).unwrap()
                ),
                expected
            );
        }
    }

    fn seeded_iterator(frame: &mut Frame<'_>, args: &CallArgs) -> crate::error::Result<PropValue> {
        let values = frame
            .actor_scope()
            .map(|scope| scope.to_vec())
            .unwrap_or_else(|| frame.arena.objects_iter().map(|(id, _)| id).collect());
        frame.begin_iterator(args, 0, values)?;
        Ok(PropValue::Object(None))
    }

    /// Registry carrying the Core arithmetic operator bodies used by the
    /// synthetic script tests (`Multiply_IntInt` at its census slot 146).
    fn registry_with_core_ops() -> NativeRegistry {
        let mut registry = NativeRegistry::new();
        let mul = crate::natives::natives_impl::lookup("Core.Object.Multiply_IntInt").unwrap();
        let add = crate::natives::natives_impl::lookup("Core.Object.Add_IntInt").unwrap();
        registry
            .register(146, 0, mul, "Core.Object.Multiply_IntInt")
            .expect("bind");
        registry
            .register(147, 0, add, "Core.Object.Subtract_IntInt")
            .expect("bind");
        let add_equal =
            crate::natives::natives_impl::lookup("Core.Object.AddEqual_FloatFloat").unwrap();
        let add_equal_int =
            crate::natives::natives_impl::lookup("Core.Object.AddEqual_IntInt").unwrap();
        let preincrement =
            crate::natives::natives_impl::lookup("Core.Object.AddAdd_PreInt").unwrap();
        let postincrement = crate::natives::natives_impl::lookup("Core.Object.AddAdd_Int").unwrap();
        let predecrement =
            crate::natives::natives_impl::lookup("Core.Object.SubtractSubtract_PreInt").unwrap();
        let postdecrement =
            crate::natives::natives_impl::lookup("Core.Object.SubtractSubtract_Int").unwrap();
        let multiply_equal =
            crate::natives::natives_impl::lookup("Core.Object.MultiplyEqual_FloatFloat").unwrap();
        registry
            .register(0x00b8, 0, add_equal, "Core.Object.AddEqual_FloatFloat")
            .expect("bind");
        registry
            .register(
                0x00b6,
                0,
                multiply_equal,
                "Core.Object.MultiplyEqual_FloatFloat",
            )
            .expect("bind");
        registry
            .register(0x00a1, 0, add_equal_int, "Core.Object.AddEqual_IntInt")
            .expect("bind");
        registry
            .register(0x00a3, 0, preincrement, "Core.Object.AddAdd_PreInt")
            .expect("bind");
        registry
            .register(0x00a5, 0, postincrement, "Core.Object.AddAdd_Int")
            .expect("bind");
        registry
            .register(
                0x00a4,
                0,
                predecrement,
                "Core.Object.SubtractSubtract_PreInt",
            )
            .expect("bind");
        registry
            .register(0x00a6, 0, postdecrement, "Core.Object.SubtractSubtract_Int")
            .expect("bind");
        registry
    }

    /// Minimal arena carrying Core script-struct templates
    /// (`("Vector", ["X","Y","Z"])` etc.) so struct constants decode.
    fn core_struct_arena(structs: &[(&str, &[&str])]) -> ObjectArena {
        let mut arena = ObjectArena::new();
        let root = arena.root_for("Core");
        for (name, fields) in structs {
            let mut children = Vec::new();
            for field in *fields {
                let index = arena.names.intern(field);
                children.push(arena.alloc(crate::arena::UObject {
                    class_id: None,
                    name_index: index,
                    outer: Some(root),
                    flags: 0,
                    data: crate::arena::ObjectData::Property(Box::new(
                        crate::arena::PropertyData {
                            kind: crate::value::PropertyKind::Int,
                            links: Default::default(),
                            array_dim: 1,
                            property_flags: 0,
                            category: 0,
                        },
                    )),
                }));
            }
            let name_index = arena.names.intern(name);
            arena.alloc(crate::arena::UObject {
                class_id: None,
                name_index,
                outer: Some(root),
                flags: 0,
                data: crate::arena::ObjectData::ScriptStruct(crate::arena::StructData {
                    children,
                    ..Default::default()
                }),
            });
        }
        arena
    }

    #[test]
    fn sleep_suspends_past_the_call_and_resumes_from_saved_pc() {
        let mut arena = ObjectArena::new();
        let mut registry = registry();
        let sleep = crate::natives::natives_impl::lookup("Engine.Actor.Sleep")
            .expect("sleep body bound by leaf name");
        registry
            .register(0x0100, 2, sleep, "Engine.Actor.Sleep")
            .expect("register");

        // Sleep(0.25); then (post-resume work) LetBool Local(7)=true; Stop.
        // EX_ExtendedNative group 1 + low 0x00 == slot 0x100.
        let mut code = vec![0x61, 0x00, USToken::FloatConst as u8];
        code.extend_from_slice(&0.25f32.to_le_bytes());
        code.push(USToken::EndFunctionParms as u8);
        code.extend_from_slice(&[
            USToken::LetBool as u8,
            USToken::LocalVariable as u8,
            7,
            USToken::IntOne as u8,
            USToken::Stop as u8,
        ]);
        let resolver = property_resolver(&mut arena, &[(7, 7)]);
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver.clone());
        frame.run().expect("run suspends cleanly");
        assert_eq!(frame.suspended(), Some(Suspend::Sleep(0.25)));
        // Cursor sits PAST the whole latent call (its EndFunctionParms),
        // exactly where a resumed run continues.
        assert_eq!(frame.pc(), 8);

        // Resume: same code stream, saved cursor — the remaining statement
        // executes and the frame completes.
        let mut resumed = Frame::with_resolver(&arena, &registry, &code, None, resolver);
        resumed.locals = frame.locals.clone();
        resumed.set_pc(frame.pc());
        resumed.run().expect("resume completes");
        assert_eq!(resumed.suspended(), None);
        assert_eq!(resumed.locals.get(7), Some(&PropValue::Bool(true)));
    }
    #[test]
    fn scalar_to_string_conversions_use_unreal_text_not_debug_syntax() {
        let arena = ObjectArena::new();
        assert_eq!(
            convert(&arena, USToken::IntToString, &PropValue::Int(7)).unwrap(),
            PropValue::Str("7".to_string())
        );
        assert_eq!(
            convert(&arena, USToken::BoolToString, &PropValue::Bool(true)).unwrap(),
            PropValue::Str("True".to_string())
        );
        assert_eq!(
            convert(&arena, USToken::FloatToString, &PropValue::Float(0.5)).unwrap(),
            PropValue::Str("0.5".to_string())
        );
        assert_eq!(
            convert(
                &arena,
                USToken::StringToFloat,
                &PropValue::Str("18.5".to_string())
            )
            .unwrap(),
            PropValue::Float(18.5)
        );
        assert_eq!(
            convert(
                &arena,
                USToken::StringToInt,
                &PropValue::Str("12".to_string())
            )
            .unwrap(),
            PropValue::Int(12)
        );
    }

    #[test]
    fn fast_trace_default_start_suspends_and_resumes_with_engine_boolean() {
        use crate::arena::{BytecodeResolver, ObjectData, PropertyData, UObject};

        let mut arena = core_struct_arena(&[("Vector", &["X", "Y", "Z"])]);
        let root = arena.root_for("Engine");
        let location_name = arena.names.intern("Location");
        let vector_name = arena.names.find_index("Vector").expect("Core.Vector");
        let x = arena.names.find_index("X").expect("Core.Vector.X");
        let y = arena.names.find_index("Y").expect("Core.Vector.Y");
        let z = arena.names.find_index("Z").expect("Core.Vector.Z");
        let actor_name = arena.names.intern("TraceSource");
        let actor = arena.alloc(UObject {
            name_index: actor_name,
            outer: Some(root),
            data: ObjectData::Properties(Default::default()),
            ..Default::default()
        });
        if let ObjectData::Properties(properties) = &mut arena.get_mut(actor).expect("actor").data {
            properties.set(
                location_name,
                PropValue::Struct {
                    struct_name: vector_name,
                    fields: vec![
                        (x, PropValue::Float(3.0)),
                        (y, PropValue::Float(4.0)),
                        (z, PropValue::Float(5.0)),
                    ],
                },
            );
        }
        let hit_name = arena.names.intern("Clear");
        let hit_property = arena.alloc(UObject {
            name_index: hit_name,
            outer: Some(root),
            data: ObjectData::Property(Box::new(PropertyData {
                links: Default::default(),
                kind: PropertyKind::Bool,
                array_dim: 1,
                property_flags: 0,
                category: 0,
            })),
            ..Default::default()
        });
        let resolver = BytecodeResolver::from_package_refs(Vec::new(), vec![actor, hit_property]);
        let mut registry = registry();
        registry
            .register(
                0x0224,
                2,
                crate::natives::natives_impl::lookup("Engine.Actor.FastTrace")
                    .expect("FastTrace"),
                "Engine.Actor.FastTrace",
            )
            .expect("register");
        let mut code = vec![
            USToken::LetBool as u8,
            USToken::LocalVariable as u8,
            2,
            0x62,
            0x24,
            USToken::VectorConst as u8,
        ];
        for component in [30.0_f32, 40.0, 50.0] {
            code.extend_from_slice(&component.to_le_bytes());
        }
        code.extend_from_slice(&[USToken::EndFunctionParms as u8, USToken::Stop as u8]);
        let mut frame = Frame::with_resolver(&arena, &registry, &code, Some(actor), resolver);
        frame.run().expect("FastTrace suspension");
        assert_eq!(
            frame.suspended(),
            Some(Suspend::EffectResult { request_id: 0 })
        );
        let effects = frame.take_effects();
        let [ScriptEffect::FastTraceRequest {
            request_id,
            source,
            start,
            end,
        }] = effects.as_slice()
        else {
            panic!("unexpected FastTrace effects {effects:?}");
        };
        assert_eq!((*request_id, *source, *start, *end), (0, actor, [3.0, 4.0, 5.0], [30.0, 40.0, 50.0]));
        frame
            .inject_effect_result(0, PropValue::Bool(true))
            .expect("engine result");
        assert_eq!(
            frame.take_suspend(),
            Some(Suspend::EffectResult { request_id: 0 })
        );
        frame.run().expect("FastTrace resume");
        assert_eq!(frame.locals.get(hit_name), Some(&PropValue::Bool(true)));
    }

    #[test]
    fn trace_suspends_for_engine_hit_and_resumes_return_and_both_out_vectors_together() {
        use crate::arena::{BytecodeResolver, ObjectData, PropertyData, UObject};

        let mut arena = core_struct_arena(&[("Vector", &["X", "Y", "Z"])]);
        let root = arena.root_for("Engine");
        let vector = arena
            .objects_iter()
            .find(|(_, object)| matches!(object.data, ObjectData::ScriptStruct(_))
                && arena.names.text(object.name_index) == Some("Vector"))
            .map(|(id, _)| id)
            .expect("Core.Vector");
        let source_name = arena.names.intern("TraceSource");
        let source = arena.alloc(UObject {
            name_index: source_name,
            outer: Some(root),
            data: ObjectData::Properties(Default::default()),
            ..Default::default()
        });
        let property = |arena: &mut ObjectArena, name: &str, kind| {
            let name_index = arena.names.intern(name);
            arena.alloc(UObject {
                name_index,
                outer: Some(root),
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
        let hit_actor = property(&mut arena, "HitActor", PropertyKind::Object { class_ref: None });
        let hit_location = property(&mut arena, "HitLocation", PropertyKind::Struct { struct_ref: Some(vector) });
        let hit_normal = property(&mut arena, "HitNormal", PropertyKind::Struct { struct_ref: Some(vector) });
        let resolver = BytecodeResolver::from_package_refs(
            Vec::new(),
            vec![source, hit_actor, hit_location, hit_normal],
        );
        let mut registry = registry();
        registry
            .register(
                0x0115,
                2,
                crate::natives::natives_impl::lookup("Engine.Actor.Trace").expect("Trace"),
                "Engine.Actor.Trace",
            )
            .expect("register");
        let vector_const = |value: [f32; 3]| {
            let mut bytes = vec![USToken::VectorConst as u8];
            for component in value {
                bytes.extend_from_slice(&component.to_le_bytes());
            }
            bytes
        };
        let mut code = vec![
            USToken::Let as u8,
            USToken::LocalVariable as u8,
            2,
            0x61,
            0x15,
            USToken::LocalVariable as u8,
            3,
            USToken::LocalVariable as u8,
            4,
        ];
        code.extend(vector_const([30.0, 40.0, 50.0]));
        code.extend(vector_const([3.0, 4.0, 5.0]));
        code.push(USToken::True as u8);
        code.extend(vector_const([0.0, 0.0, 0.0]));
        code.extend_from_slice(&[USToken::EndFunctionParms as u8, USToken::Stop as u8]);
        let mut frame = Frame::with_resolver(&arena, &registry, &code, Some(source), resolver);

        frame.run().expect("Trace suspension");
        assert_eq!(frame.suspended(), Some(Suspend::EffectResult { request_id: 0 }));
        let effects = frame.take_effects();
        assert!(matches!(
            effects.as_slice(),
            [ScriptEffect::TraceRequest {
                request_id: 0,
                source: request_source,
                start,
                end,
                trace_actors: true,
                extent,
            }] if *request_source == source
                && *start == [3.0, 4.0, 5.0]
                && *end == [30.0, 40.0, 50.0]
                && *extent == [0.0, 0.0, 0.0]
        ));
        let hit = ObjectId(91);
        let vector_value = |value: [f32; 3]| PropValue::Struct {
            struct_name: arena.names.find_index("Vector").expect("Vector"),
            fields: vec![
                (arena.names.find_index("X").expect("X"), PropValue::Float(value[0])),
                (arena.names.find_index("Y").expect("Y"), PropValue::Float(value[1])),
                (arena.names.find_index("Z").expect("Z"), PropValue::Float(value[2])),
            ],
        };
        frame
            .inject_effect_result(
                0,
                PropValue::FixedArray(vec![
                    PropValue::Object(Some(hit.0 as i32)),
                    vector_value([12.0, 13.0, 14.0]),
                    vector_value([-1.0, 0.0, 1.0]),
                ]),
            )
            .expect("engine Trace result");
        frame.run().expect("Trace resume");
        assert_eq!(frame.locals.get(arena.names.find_index("HitActor").unwrap()), Some(&PropValue::Object(Some(hit.0 as i32))));
        assert_eq!(frame.locals.get(arena.names.find_index("HitLocation").unwrap()), Some(&vector_value([12.0, 13.0, 14.0])));
        assert_eq!(frame.locals.get(arena.names.find_index("HitNormal").unwrap()), Some(&vector_value([-1.0, 0.0, 1.0])));
    }

    #[test]
    fn effect_result_resumes_conversion_before_conditional_parent() {
        use crate::arena::{ClassData, PropertyData, UObject};

        let mut arena = ObjectArena::new();
        let root = arena.root_for("Engine");
        let class_name = arena.names.intern("Actor");
        let class = arena.alloc(UObject {
            name_index: class_name,
            outer: Some(root),
            data: ObjectData::Class(ClassData::default()),
            ..Default::default()
        });
        let local_name = arena.names.intern("Reached");
        let property = arena.alloc(UObject {
            name_index: local_name,
            outer: Some(class),
            data: ObjectData::Property(Box::new(PropertyData {
                links: Default::default(),
                kind: PropertyKind::Bool,
                array_dim: 1,
                property_flags: 0,
                category: 0,
            })),
            ..Default::default()
        });
        let resolver = BytecodeResolver::from_package_refs(Vec::new(), vec![class, property]);
        let mut registry = registry();
        registry
            .register(
                0x0116,
                2,
                crate::natives::natives_impl::lookup("Engine.Actor.Spawn").expect("Spawn"),
                "Engine.Actor.Spawn",
            )
            .expect("register");
        let code = vec![
            USToken::JumpIfNot as u8,
            0,
            0,
            USToken::ObjectToBool as u8,
            0x61,
            0x16,
            USToken::ObjectConst as u8,
            1,
            USToken::EndFunctionParms as u8,
            USToken::LetBool as u8,
            USToken::LocalVariable as u8,
            2,
            USToken::IntOne as u8,
            USToken::Stop as u8,
        ];
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver);

        frame.run().expect("Spawn suspension");
        assert_eq!(
            frame.suspended(),
            Some(Suspend::EffectResult { request_id: 0 })
        );
        let reserved = match frame.take_effects().as_slice() {
            [ScriptEffect::SpawnRequest { reserved, .. }] => *reserved,
            effects => panic!("unexpected effects {effects:?}"),
        };
        frame
            .inject_effect_result(
                0,
                PropValue::Object(Some(i32::try_from(reserved.0).unwrap())),
            )
            .expect("inject");
        assert_eq!(
            frame.take_suspend(),
            Some(Suspend::EffectResult { request_id: 0 })
        );
        frame.run().expect("conversion and conditional resume");

        assert_eq!(frame.locals.get(local_name), Some(&PropValue::Bool(true)));
    }

    #[test]
    fn latent_sleep_inside_let_keeps_s2_assignment_semantics() {
        let mut arena = ObjectArena::new();
        let mut registry = registry();
        let sleep = crate::natives::natives_impl::lookup("Engine.Actor.Sleep").expect("sleep");
        registry
            .register(0x0100, 2, sleep, "Engine.Actor.Sleep")
            .expect("register");
        let mut code = vec![
            USToken::Let as u8,
            USToken::LocalVariable as u8,
            7,
            0x61,
            0x00,
            USToken::FloatConst as u8,
        ];
        code.extend_from_slice(&0.25f32.to_le_bytes());
        code.extend_from_slice(&[USToken::EndFunctionParms as u8, USToken::Stop as u8]);
        let resolver = property_resolver(&mut arena, &[(7, 7)]);
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver);

        frame.run().expect("Sleep in Let parks cleanly");

        assert_eq!(frame.suspended(), Some(Suspend::Sleep(0.25)));
        assert_eq!(frame.locals.get(7), Some(&PropValue::Int(0)));
        assert!(
            frame.pending_expression.is_none(),
            "latent S2 suspension must not use effect-result continuation storage"
        );
    }

    #[test]
    fn finish_anim_suspends_past_its_call() {
        let arena = ObjectArena::new();
        let mut registry = registry();
        let finish =
            crate::natives::natives_impl::lookup("Engine.Actor.FinishAnim").expect("bound");
        registry
            .register(0x0105, 2, finish, "Engine.Actor.FinishAnim")
            .expect("register");
        let code = [
            0x61,
            0x05,
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        frame.run().expect("FinishAnim suspends");
        assert_eq!(frame.suspended(), Some(Suspend::Anim));
        assert_eq!(frame.pc(), 3);
    }

    #[test]
    fn finish_interpolation_suspends_past_native_slot_301_call() {
        let arena = ObjectArena::new();
        let mut registry = registry();
        let finish = crate::natives::natives_impl::lookup("Engine.Actor.FinishInterpolation")
            .expect("bound");
        registry
            .register(0x012D, 2, finish, "Engine.Actor.FinishInterpolation")
            .expect("register");
        let code = [
            0x61,
            0x2D,
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);

        frame.run().expect("FinishInterpolation suspends");

        assert_eq!(frame.suspended(), Some(Suspend::Interpolation));
        assert_eq!(frame.pc(), 3);
    }

    #[test]
    fn goto_state_stops_at_its_call_boundary_after_prior_requests() {
        let mut arena = ObjectArena::new();
        let mut registry = registry();
        let goto_state =
            crate::natives::natives_impl::lookup("Core.Object.GotoState").expect("bound");
        let set_timer =
            crate::natives::natives_impl::lookup("Engine.Actor.SetTimer").expect("bound");
        registry
            .register(0x71, 0, goto_state, "Core.Object.GotoState")
            .expect("register");
        registry
            .register(0x0118, 2, set_timer, "Engine.Actor.SetTimer")
            .expect("register");
        let running = arena.names.intern("Running") as u8;
        // SetTimer(0.25, true); GotoState('Running'); Stop. The timer is
        // retained, while the Stop must not execute in the old activation.
        let mut code = vec![0x61, 0x18]; // slot 0x118
        code.push(USToken::FloatConst as u8);
        code.extend_from_slice(&0.25f32.to_le_bytes());
        code.extend_from_slice(&[USToken::True as u8, USToken::EndFunctionParms as u8]);
        code.extend_from_slice(&[0x71, USToken::NameConst as u8, running]);
        code.extend_from_slice(&[USToken::EndFunctionParms as u8, USToken::Stop as u8]);
        let actor = arena.alloc(crate::arena::UObject::default());
        let mut frame = Frame::new(&arena, &registry, &code, Some(actor));
        frame.run().expect("run");
        assert_eq!(frame.suspended(), Some(Suspend::StateChange));
        assert_eq!(frame.pc(), code.len() - 1, "cursor is past GotoState");
        assert_eq!(
            frame.take_latent_requests(),
            vec![
                LatentRequest::SetTimer {
                    actor,
                    seconds: 0.25,
                    repeating: true
                },
                LatentRequest::GotoState {
                    actor,
                    change: StateChange::Named("Running".into()),
                },
            ]
        );
    }

    #[test]
    fn enable_and_disable_queue_probe_dispatch_changes() {
        let mut arena = ObjectArena::new();
        let mut registry = registry();
        let enable = crate::natives::natives_impl::lookup("Core.Object.Enable").expect("bound");
        let disable = crate::natives::natives_impl::lookup("Core.Object.Disable").expect("bound");
        registry
            .register(0x75, 0, enable, "Core.Object.Enable")
            .expect("register");
        registry
            .register(0x76, 0, disable, "Core.Object.Disable")
            .expect("register");
        let timer = arena.names.intern("Timer") as u8;
        let code = [
            0x75,
            USToken::NameConst as u8,
            timer,
            USToken::EndFunctionParms as u8,
            0x76,
            USToken::NameConst as u8,
            timer,
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ];
        let actor = arena.alloc(crate::arena::UObject::default());
        let mut frame = Frame::new(&arena, &registry, &code, Some(actor));
        frame.run().expect("probe natives execute");
        assert_eq!(
            frame.take_latent_requests(),
            vec![
                LatentRequest::SetEventEnabled {
                    actor,
                    event: "Timer".into(),
                    enabled: true,
                },
                LatentRequest::SetEventEnabled {
                    actor,
                    event: "Timer".into(),
                    enabled: false,
                },
            ]
        );
    }

    #[test]
    fn malformed_parameter_jump_cannot_loop_or_grow_arguments() {
        let arena = ObjectArena::new();
        let registry = registry();
        // Numbered native 0x70 begins a parameter list. Its first
        // expression is EX_Jump back to its own opcode, so the per-call
        // forward-progress guard must fail before a native lookup or
        // argument allocation loop can occur.
        let code = [0x70, USToken::Jump as u8, 1, 0];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        assert_eq!(
            frame
                .run()
                .expect_err("backward parameter jump must abort")
                .reason_code,
            "vm.parm_progress_stalled"
        );
    }

    #[test]
    fn get_state_name_and_is_in_state_read_the_embedded_declaration() {
        let mut arena = ObjectArena::new();
        let mut registry = registry();
        let get_state_name =
            crate::natives::natives_impl::lookup("Engine.Actor.GetStateName").expect("bound");
        let is_in_state =
            crate::natives::natives_impl::lookup("Engine.Actor.IsInState").expect("bound");
        registry
            .register(0x011C, 2, get_state_name, "Engine.Actor.GetStateName")
            .expect("register");
        registry
            .register(0x0119, 2, is_in_state, "Engine.Actor.IsInState")
            .expect("register");
        let running = arena.names.intern("Running");
        // GetStateName() as the return expression (EX_Return carries one).
        let code = [
            USToken::Return as u8,
            0x61,
            0x1C,
            USToken::EndFunctionParms as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        frame.set_active_state(Some(running));
        match frame.run().expect("run") {
            PropValue::Name(name) => assert_eq!(name.index, running),
            other => panic!("expected Name, got {other:?}"),
        }
        // IsInState('Running') true / IsInState('Other') false.
        let hit = {
            let mut c = vec![USToken::Return as u8, 0x61, 0x19, USToken::NameConst as u8];
            c.push(running as u8);
            c.push(USToken::EndFunctionParms as u8);
            c
        };
        let mut frame = Frame::new(&arena, &registry, &hit, None);
        frame.set_active_state(Some(running));
        assert_eq!(frame.run().expect("run"), PropValue::Bool(true));
        let miss = {
            let other = arena.names.intern("Other") as u8;
            let mut c = vec![
                USToken::Return as u8,
                0x61,
                0x19,
                USToken::NameConst as u8,
                other,
            ];
            c.push(USToken::EndFunctionParms as u8);
            c
        };
        let mut frame = Frame::new(&arena, &registry, &miss, None);
        frame.set_active_state(Some(running));
        assert_eq!(frame.run().expect("run"), PropValue::Bool(false));
        // No declared state: comparisons fail closed.
        let mut frame = Frame::new(&arena, &registry, &hit, None);
        assert_eq!(frame.run().expect("run"), PropValue::Bool(false));
    }

    #[test]
    fn nested_call_suspension_parks_the_whole_chain() {
        use crate::arena::{ClassData, FunctionData, UObject};
        let mut arena = ObjectArena::new();
        let mut registry = registry();
        let sleep = crate::natives::natives_impl::lookup("Engine.Actor.Sleep").expect("bound");
        registry
            .register(0x0100, 2, sleep, "Engine.Actor.Sleep")
            .expect("register");
        let root = arena.root_for("Engine");
        let class = {
            let name_index = arena.names.intern("Sleeper");
            arena.alloc(UObject {
                class_id: None,
                name_index,
                outer: Some(root),
                flags: 0,
                data: ObjectData::Class(ClassData::default()),
            })
        };
        // Callee(): Sleep(0.5); Resumed = 1; Stop. The write after Sleep
        // must run before the caller resumes at its own Stop.
        let resumed = arena.names.intern("Resumed");
        let mut callee_code = vec![0x61, 0x00, USToken::FloatConst as u8];
        callee_code.extend_from_slice(&0.5f32.to_le_bytes());
        callee_code.push(USToken::EndFunctionParms as u8);
        callee_code.extend_from_slice(&[
            USToken::Let as u8,
            USToken::InstanceVariable as u8,
            resumed as u8,
            USToken::IntOne as u8,
            USToken::Stop as u8,
        ]);
        let resolver = property_resolver(&mut arena, &[(resumed as i32, resumed)]);
        {
            let name_index = arena.names.intern("Callee");
            arena.alloc(UObject {
                class_id: None,
                name_index,
                outer: Some(class),
                flags: 0,
                data: ObjectData::Function(Box::new(FunctionData {
                    code: callee_code,
                    resolver,
                    ..Default::default()
                })),
            });
        }
        let actor = {
            let name_index = arena.names.intern("Actor");
            arena.alloc(UObject {
                class_id: Some(class),
                name_index,
                outer: Some(root),
                flags: 0,
                data: ObjectData::Properties(PropStore::new()),
            })
        };
        // Caller: Callee(); Stop.
        let callee_index = arena.names.find_index("Callee").expect("name") as u8;
        let caller_code = [
            USToken::VirtualFunction as u8,
            callee_index,
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &caller_code, Some(actor));
        frame.run().expect("suspension propagates as a clean yield");
        assert_eq!(
            frame.suspended(),
            Some(Suspend::Sleep(0.5)),
            "the callee's Sleep must park the parent activation"
        );
        assert_eq!(frame.pc(), 3, "parent cursor rests past the call site");
        // Reinstall the call snapshot just taken, clear the scheduler wake
        // marker, and resume the same outer frame. Engine does this across
        // its persisted ScriptFrame boundary.
        let call = frame
            .take_suspended_call()
            .expect("the yielded callee activation is persisted");
        frame.restore_suspended_call(call);
        assert_eq!(frame.take_suspend(), Some(Suspend::Sleep(0.5)));
        frame.run().expect("callee post-Sleep work resumes");
        assert_eq!(
            frame.take_instance_writes(),
            vec![InstanceWrite {
                object: actor,
                name: resumed,
                value: PropValue::Int(1),
            }]
        );
    }
    #[test]
    fn nested_log_effects_share_the_parent_budget() {
        let arena = ObjectArena::new();
        let registry = registry();
        let mut parent = Frame::new(&arena, &registry, &[], None);
        let mut nested = Frame::new(&arena, &registry, &[], None);
        for index in 0..128 {
            parent.log_effect(format!("parent-{index}")).expect("parent log");
        }
        for index in 0..257 {
            nested.log_effect(format!("nested-{index}")).expect("nested log");
        }

        parent.absorb_nested_effects(&mut nested);

        let effects = parent.take_effects();
        assert_eq!(
            effects
                .iter()
                .filter(|effect| matches!(effect, ScriptEffect::Log { .. }))
                .count(),
            256
        );
        assert_eq!(
            effects.last(),
            Some(&ScriptEffect::Log {
                message: "nested-127".to_string(),
            })
        );
    }


    #[test]
    fn suspended_context_keeps_lexical_self_separate_from_dispatch_target() {
        use crate::arena::UObject;

        let mut arena = ObjectArena::new();
        let root = arena.root_for("Engine");
        let caller_name = arena.names.intern("Caller");
        let caller = arena.alloc(UObject {
            class_id: None,
            name_index: caller_name,
            outer: Some(root),
            flags: 0,
            data: ObjectData::Properties(PropStore::new()),
        });
        let target_name = arena.names.intern("ContextTarget");
        let target = arena.alloc(UObject {
            class_id: None,
            name_index: target_name,
            outer: Some(root),
            flags: 0,
            data: ObjectData::Properties(PropStore::new()),
        });
        let registry = registry();
        let mut frame = Frame::new(&arena, &registry, &[USToken::Stop as u8], Some(caller));
        frame.self_id = Some(target);

        let snapshot = frame.snapshot_suspended_call();
        let resumed = Frame::from_suspended_call(
            &arena,
            &registry,
            snapshot,
            None,
            Vec::new(),
        );

        assert_eq!(resumed.self_id, Some(target));
        assert_eq!(resumed.lexical_self_id, Some(caller));
    }
    #[test]
    fn suspended_nested_compound_write_is_visible_and_survives_resume() {
        use crate::arena::{ClassData, FunctionData, UObject};
        let mut arena = ObjectArena::new();
        let mut registry = registry_with_core_ops();
        let sleep = crate::natives::natives_impl::lookup("Engine.Actor.Sleep").unwrap();
        registry
            .register(0x0100, 2, sleep, "Engine.Actor.Sleep")
            .unwrap();
        let root = arena.root_for("Engine");
        let class_name = arena.names.intern("CompoundSleeper");
        let class = arena.alloc(UObject {
            name_index: class_name,
            outer: Some(root),
            data: ObjectData::Class(ClassData::default()),
            ..Default::default()
        });
        let value_name = arena.names.intern("Value");
        let mut callee_code = vec![
            0xb8,
            USToken::InstanceVariable as u8,
            1,
            USToken::FloatConst as u8,
        ];
        callee_code.extend_from_slice(&2.0f32.to_le_bytes());
        callee_code.extend_from_slice(&[
            USToken::EndFunctionParms as u8,
            0x61,
            0x00,
            USToken::FloatConst as u8,
        ]);
        callee_code.extend_from_slice(&0.5f32.to_le_bytes());
        callee_code.extend_from_slice(&[
            USToken::EndFunctionParms as u8,
            0xb8,
            USToken::InstanceVariable as u8,
            1,
            USToken::FloatConst as u8,
        ]);
        callee_code.extend_from_slice(&4.0f32.to_le_bytes());
        callee_code.extend_from_slice(&[USToken::EndFunctionParms as u8, USToken::Stop as u8]);
        let resolver = property_resolver(&mut arena, &[(1, value_name)]);
        let callee_name = arena.names.intern("CalleeCompound");
        arena.alloc(UObject {
            name_index: callee_name,
            outer: Some(class),
            data: ObjectData::Function(Box::new(FunctionData {
                code: callee_code,
                resolver,
                ..Default::default()
            })),
            ..Default::default()
        });
        let actor_name = arena.names.intern("Actor");
        let actor = arena.alloc(UObject {
            name_index: actor_name,
            class_id: Some(class),
            outer: Some(root),
            data: ObjectData::Properties({
                let mut store = PropStore::new();
                store.set(value_name, PropValue::Float(1.0));
                store
            }),
            ..Default::default()
        });
        let outer_name = arena.names.intern("Outer");
        let caller_code = [
            USToken::Let as u8,
            USToken::InstanceVariable as u8,
            2,
            USToken::VirtualFunction as u8,
            callee_name as u8,
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ];
        let caller_resolver = property_resolver(&mut arena, &[(2, outer_name)]);
        let mut frame = Frame::with_resolver(
            &arena,
            &registry,
            &caller_code,
            Some(actor),
            caller_resolver,
        );
        frame.run().expect("nested compound reaches Sleep");
        assert_eq!(
            frame.overlay_value(actor, value_name),
            Some(PropValue::Float(3.0))
        );
        assert_eq!(
            frame.overlay_value(actor, outer_name),
            None,
            "outer assignment must wait for the suspended script call"
        );
        let call = frame.take_suspended_call().unwrap();
        frame.restore_suspended_call(call);
        assert_eq!(frame.take_suspend(), Some(Suspend::Sleep(0.5)));
        frame.run().expect("nested compound resumes");
        assert_eq!(
            frame.overlay_value(actor, value_name),
            Some(PropValue::Float(7.0))
        );
        assert_eq!(
            frame.overlay_value(actor, outer_name),
            Some(PropValue::Float(7.0)),
            "outer assignment receives the resumed callee result"
        );
        assert_eq!(
            frame.take_instance_writes(),
            vec![
                InstanceWrite {
                    object: actor,
                    name: value_name,
                    value: PropValue::Float(3.0),
                },
                InstanceWrite {
                    object: actor,
                    name: value_name,
                    value: PropValue::Float(7.0),
                },
                InstanceWrite {
                    object: actor,
                    name: outer_name,
                    value: PropValue::Float(7.0),
                },
            ]
        );
    }

    #[test]
    fn state_entry_pc_requires_a_readable_begin_label() {
        let mut arena = ObjectArena::new();
        let _none = arena.names.intern("None");
        let begin = arena.names.intern("Begin");
        let idle = arena.names.intern("Idle");
        let resolver = BytecodeResolver::default();

        let mut code = vec![USToken::Stop as u8, USToken::LabelTable as u8];
        let table_at = 2;
        code.push(begin as u8);
        code.extend_from_slice(&0i32.to_le_bytes());
        code.push(idle as u8);
        code.extend_from_slice(&0i32.to_le_bytes());
        code.push(0);
        assert_eq!(
            state_entry_pc(&arena, &resolver, &code, table_at).expect("Begin"),
            0
        );

        let mut code = vec![
            USToken::Nothing as u8,
            USToken::Stop as u8,
            USToken::LabelTable as u8,
        ];
        let table_at = 3;
        code.push(idle as u8);
        code.extend_from_slice(&0i32.to_le_bytes());
        code.push(begin as u8);
        code.extend_from_slice(&1i32.to_le_bytes());
        code.push(0);
        assert_eq!(
            state_entry_pc(&arena, &resolver, &code, table_at).expect("Begin"),
            1
        );

        let mut code = vec![USToken::Stop as u8, USToken::LabelTable as u8];
        let table_at = 2;
        code.push(idle as u8);
        code.extend_from_slice(&0i32.to_le_bytes());
        code.push(0);
        assert_eq!(
            state_entry_pc(&arena, &resolver, &code, table_at)
                .expect_err("missing Begin must fail")
                .reason_code,
            "vm.state_begin_missing"
        );

        let code = [USToken::Stop as u8];
        assert_eq!(
            state_entry_pc(&arena, &resolver, &code, 1)
                .expect_err("invalid table offset must fail")
                .reason_code,
            "vm.state_label_table_offset"
        );
    }
    /// The interpreter's opcode dispatch is exhaustive: `exec` matches every
    /// [`USToken`] variant with no wildcard arm (compiler-enforced), so the
    /// only failure mode for a byte is "decodes to nothing".
    #[test]
    fn token_table_matches_unstack_h_exactly() {
        const DEFINED: [u8; 90] = [
            0x00, 0x01, 0x02, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C,
            0x1D, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A,
            0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x32, 0x33, 0x34, 0x36, 0x37, 0x38, 0x39, 0x3A,
            0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
            0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56,
            0x57, 0x58, 0x59, 0x5A, 0x60, 0x70,
        ];
        let mut expected = [false; 256];
        for &opcode in &DEFINED {
            expected[opcode as usize] = true;
        }
        for opcode in 0..=255u8 {
            assert_eq!(
                USToken::from_opcode(opcode).is_some(),
                expected[opcode as usize],
                "opcode {opcode:#04x} disagrees with UnStack.h EExprToken"
            );
        }
    }

    #[test]
    fn unknown_token_fails_loudly() {
        // Opcodes at/above EX_FirstNative are numbered-native slots; a
        // dangling call still fails loudly, never silent progress.
        let arena = ObjectArena::new();
        let registry = registry();
        let mut frame = Frame::new(&arena, &registry, &[0xFF], None);
        let fail = frame
            .run()
            .expect_err("dangling native call must not execute");
        assert_eq!(fail.reason_code, "vm.code_truncated");
        // A fork-undocumented gap opcode (UnStack.h defines nothing at
        // 0x03/0x35/0x5B..0x6F) is loud under its deferrable reason.
        let mut frame = Frame::new(&arena, &registry, &[USToken::IntZero as u8, 0x03], None);
        let fail = frame.run().expect_err("0x03 must not execute");
        assert_eq!(fail.reason_code, "vm.unknown_token_deferrable");
    }

    #[test]
    fn switch_dispatches_to_matched_case_body() {
        // switch (2) { case 1: return 10; case 2: return 20; }
        // Record layout: [Case][next u16 pointing at the NEXT Case][expr].
        let arena = ObjectArena::new();
        let registry = registry();
        let mut code = vec![USToken::Switch as u8, 4]; // raw-width compare
        code.push(USToken::IntConst as u8);
        code.extend_from_slice(&2i32.to_le_bytes());
        let case1_at = code.len();
        code.push(USToken::Case as u8);
        code.extend_from_slice(&[0, 0]); // patched below
        code.push(USToken::IntConst as u8);
        code.extend_from_slice(&1i32.to_le_bytes());
        code.push(USToken::Return as u8); // case 1 body
        code.push(USToken::IntConst as u8);
        code.extend_from_slice(&10i32.to_le_bytes());
        let case2_at = code.len();
        code.push(USToken::Case as u8);
        code.extend_from_slice(&[0xFF, 0xFF]); // default terminator
        code.push(USToken::IntConst as u8);
        code.extend_from_slice(&2i32.to_le_bytes());
        code.push(USToken::Return as u8); // case 2 body (default here)
        code.push(USToken::IntConst as u8);
        code.extend_from_slice(&20i32.to_le_bytes());
        let patch = |c: &mut Vec<u8>, at: usize, next: usize| {
            c[at + 1..at + 3].copy_from_slice(&(next as u16).to_le_bytes());
        };
        patch(&mut code, case1_at, case2_at);

        let mut frame = Frame::new(&arena, &registry, &code, None);
        let value = frame.run().expect("switch executes");
        assert_eq!(value, PropValue::Int(20));
    }
    #[test]
    fn switch_dispatches_to_matched_string_case_body() {
        let mut arena = ObjectArena::new();
        let object = arena.alloc(crate::arena::UObject::default());
        let resolver = resolver_with_objects(&mut arena, &[(1, object)]);
        let registry = registry();
        let string = |code: &mut Vec<u8>, value: &str| {
            code.push(USToken::StringConst as u8);
            code.extend_from_slice(value.as_bytes());
            code.push(0);
        };
        let mut code = vec![USToken::ObjectConst as u8, 1, USToken::Switch as u8, 0];
        string(&mut code, "TRIGGER");
        let first_case = code.len();
        code.extend_from_slice(&[USToken::Case as u8, 0, 0]);
        string(&mut code, "CAPTURE");
        code.extend_from_slice(&[USToken::Return as u8, USToken::IntOne as u8]);
        let trigger_case = code.len();
        code.extend_from_slice(&[USToken::Case as u8, 0, 0]);
        string(&mut code, "TRIGGER");
        code.extend_from_slice(&[USToken::Return as u8, USToken::IntConst as u8, 2, 0, 0, 0]);
        let default_case = code.len();
        code.extend_from_slice(&[USToken::Case as u8, 0xFF, 0xFF]);
        code.extend_from_slice(&[USToken::Return as u8, USToken::IntConst as u8, 3, 0, 0, 0]);
        code[first_case + 1..first_case + 3].copy_from_slice(&((trigger_case + 3) as u16).to_le_bytes());
        code[trigger_case + 1..trigger_case + 3].copy_from_slice(&((default_case + 3) as u16).to_le_bytes());

        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver);
        assert_eq!(frame.run().expect("string switch executes"), PropValue::Int(2));
    }


    #[test]
    fn int_const_return_program_runs() {
        let arena = ObjectArena::new();
        let registry = registry();
        // IntConst(42); Return.
        let code = [
            USToken::IntConst as u8,
            42,
            0,
            0,
            0,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        assert_eq!(frame.run().expect("run"), PropValue::Int(42));
    }

    #[test]
    fn let_bool_stores_through_lvalue() {
        let mut arena = ObjectArena::new();
        let registry = registry();
        // LetBool <lvalue: LocalVariable(7)> <expr: IntOne>; Return Nothing.
        let code = [
            USToken::LetBool as u8,
            USToken::LocalVariable as u8,
            7,
            USToken::IntOne as u8,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let resolver = property_resolver(&mut arena, &[(7, 7)]);
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver);
        frame.run().expect("run");
        assert_eq!(frame.locals.get(7), Some(&PropValue::Bool(true)));
    }

    #[test]
    fn jump_if_not_branches_on_local() {
        let mut arena = ObjectArena::new();
        let registry = registry();
        //  0: JumpIfNot -> 8      (expr: LocalVariable(7))
        //  3: IntOne; 4: LocalVariable(7); 6: Return; 7: Nothing
        //  8: IntZero; 9: LocalVariable(7); 11: Return; 12: Nothing
        let code = [
            USToken::JumpIfNot as u8,
            8,
            0,
            USToken::IntOne as u8,
            USToken::LocalVariable as u8,
            7,
            USToken::Return as u8,
            USToken::Nothing as u8,
            USToken::IntZero as u8,
            USToken::LocalVariable as u8,
            7,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        // False local: branch taken, falls into the zero/false leg.
        let resolver = property_resolver(&mut arena, &[(7, 7)]);
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver.clone());
        frame.locals.set(7, PropValue::Bool(false));
        assert_eq!(frame.run().expect("run"), PropValue::Bool(false));
        // True local: branch skipped, the one-leg wins.
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver);
        frame.locals.set(7, PropValue::Bool(true));
        assert_eq!(frame.run().expect("run"), PropValue::Bool(true));
    }

    #[test]
    fn extended_native_slot_spans_opcode_nibble_and_byte() {
        use crate::value::PropValue as PV;
        let arena = ObjectArena::new();
        let mut registry = registry();
        fn body(
            frame: &mut Frame<'_>,
            _args: &crate::natives::CallArgs,
        ) -> crate::error::Result<PV> {
            let seeded = matches!(frame.locals.get(7), Some(PV::Int(5)));
            Ok(PV::Int(i32::from(seeded)))
        }
        // Slot 2983 = 0xBA7: opcode 0x6B (group 0xB) + byte 0xA7, dispatched
        // through run()'s interception path.
        registry
            .register(0x0BA7, 0, body, "probe.extended")
            .expect("register");
        let code = [
            0x6B,
            0xA7,
            USToken::EndFunctionParms as u8,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        frame.locals.set(7, PropValue::Int(5));
        assert_eq!(frame.run().expect("run"), PropValue::Int(1));

        // Slot 167 = 0xA7 via the EX_ExtendedNative token itself
        // (opcode 0x60, group 0), reached through expr() nesting.
        registry
            .register(167, 0, body, "probe.group0")
            .expect("register");
        let code = [
            USToken::EatString as u8,
            0x60,
            0xA7,
            USToken::EndFunctionParms as u8,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        frame.locals.set(7, PropValue::Int(5));
        assert_eq!(frame.run().expect("run"), PropValue::Int(1));

        // An unregistered slot fails loudly at dispatch.
        let code = [
            0x6C,
            0x00,
            USToken::EndFunctionParms as u8,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        let fail = frame.run().expect_err("unbound slot");
        assert_eq!(fail.reason_code, "native.slot_unbound");
    }

    #[test]
    fn actor_error_slot_233_dispatches_actor_failure_signal() {
        let arena = ObjectArena::new();
        let mut registry = registry();
        registry
            .register(
                233,
                2,
                crate::natives::natives_impl::lookup("Engine.Actor.Error")
                    .expect("Actor.Error native"),
                "Engine.Actor.Error",
            )
            .expect("register Actor.Error");
        let code = [
            233,
            USToken::StringConst as u8,
            b't',
            b'e',
            b'r',
            b'm',
            b'i',
            b'n',
            b'a',
            b'l',
            0,
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        let fail = frame
            .run()
            .expect_err("Actor.Error must terminate the receiving actor frame");
        assert_eq!(fail.reason_code, "engine.actor_error");
        assert_eq!(fail.message, "terminal");
    }

    #[test]
    fn operand_widths_match_fork_runtime() {
        let mut arena = ObjectArena::new();
        let registry = registry();

        // Assert: _WORD line then expression — no guard byte. If a guard
        // byte were consumed, True would be eaten and the Return would read
        // Nothing with a stale result instead of Bool(true).
        let code = [
            USToken::Assert as u8,
            0x10,
            0x00,
            USToken::True as u8,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        assert_eq!(frame.run().expect("run"), PropValue::Bool(true));

        // LineNumber: _WORD line. A compact-index misread would swallow one
        // byte and desync into LocalVariable.
        let code = [
            USToken::LineNumber as u8,
            0x02,
            0x00,
            USToken::True as u8,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        assert_eq!(frame.run().expect("run"), PropValue::Bool(true));

        // DebugInfo: INT version + INT line + INT pos + NUL-terminated tag;
        // executes as a consumed no-op (HANDLE_OPTIONAL_DEBUG_INFO).
        let code = [
            USToken::DebugInfo as u8,
            100,
            0,
            0,
            0, // version = 100
            7,
            0,
            0,
            0, // line
            3,
            0,
            0,
            0, // pos
            b'O',
            b'K',
            0,
            USToken::True as u8,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        assert_eq!(frame.run().expect("run"), PropValue::Bool(true));

        // Context/ClassContext: object expr, _WORD RELATIVE null-skip,
        // BYTE size, member expr. A null context object takes the skip
        // (UE execNullContext `Code += wSkip`) — here it skips the IntOne
        // member and lands on Return, yielding the pre-context Int(0).
        let code = [
            USToken::Context as u8,
            USToken::IntZero as u8,
            1,
            0,
            1,
            USToken::IntOne as u8,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        assert_eq!(frame.run().expect("run"), PropValue::Int(0));

        // Real BaseCam.StateCutSceneCam.BeginState nests a null Context as
        // another Context's object expression. The inner member is an
        // InstanceVariable whose two serialized compact-index bytes expand
        // to a four-byte runtime UObject slot. Its skip target is therefore
        // the expression-end coordinate between that member and the outer
        // Context's WORD operand, not an opcode coordinate.
        let member_name = arena.names.intern("NestedMember");
        let resolver = property_resolver(&mut arena, &[(104, member_name)]);
        let code = [
            USToken::Context as u8,
            USToken::Context as u8,
            USToken::IntZero as u8,
            5,
            0,
            4,
            USToken::InstanceVariable as u8,
            0x68,
            0x01,
            1,
            0,
            1,
            USToken::IntOne as u8,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver);
        assert_eq!(frame.run().expect("nested null context"), PropValue::Int(0));
        // A null-skip landing outside the code is a loud truncation, never
        // a silent wrap (negative and overflow directions both checked).
        for skip in [-100_i16, 300_i16] {
            let code = [
                USToken::Context as u8,
                USToken::IntZero as u8,
                (skip & 0xff) as u8,
                ((skip >> 8) & 0xff) as u8,
                1,
                USToken::IntOne as u8,
                USToken::Return as u8,
                USToken::Nothing as u8,
            ];
            let mut frame = Frame::new(&arena, &registry, &code, None);
            let fail = frame.run().expect_err("out-of-code skip must fail");
            assert_eq!(fail.reason_code, "vm.code_truncated");
        }

        // Iterator: expression FIRST, then the runtime Script end offset.
        // A non-iterator expression is a loud native-cursor failure.
        let code = [
            USToken::Iterator as u8,
            USToken::IntZero as u8,
            0x04,
            0x00,
            USToken::IteratorPop as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        let fail = frame
            .run()
            .expect_err("iterator expression must initialize a cursor");
        assert_eq!(fail.reason_code, "native.iterator_uninitialized");
    }
    #[test]
    fn iterator_native_binds_all_candidates_in_arena_order() {
        let mut arena = ObjectArena::new();
        let first = arena.alloc(crate::arena::UObject::default());
        let second = arena.alloc(crate::arena::UObject::default());
        let out = arena.names.intern("OutActor");
        assert_eq!(out, 0, "name index remains separate from raw object refs");
        let mut registry = registry();
        registry
            .register(0x70, 0, seeded_iterator, "Test.SeededIterator")
            .expect("register iterator native");
        let code = [
            USToken::Iterator as u8,
            0x70,
            USToken::LocalVariable as u8,
            1,
            USToken::EndFunctionParms as u8,
            11,
            0,
            USToken::IteratorNext as u8,
            USToken::IteratorPop as u8,
        ];
        let resolver = property_resolver(&mut arena, &[(1, out)]);
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver);
        frame.set_actor_scope(&[first, second]);
        frame.run().expect("iterate");
        assert_eq!(
            frame.locals.get(out),
            Some(&PropValue::Object(Some(second.0 as i32)))
        );
        assert_ne!(first, second);
    }

    #[test]
    fn fork_bool_variable_redispatches_the_following_expr() {
        // `execBoolVariable` (UnCorSc.cpp:429) consumes ONE byte and calls
        // `GNatives[B]`; the compiler writes EX_BoolVariable before a plain
        // variable load (UnScrCom.cpp:1637). The bitmask lives in the bound
        // UBoolProperty, never in code — the old compact-index + mask-byte
        // reading desynced shipped bodies (retail Mover.Tick crash).
        let mut arena = ObjectArena::new();
        let registry = registry();
        let code = [
            USToken::BoolVariable as u8,
            USToken::LocalVariable as u8,
            7,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let resolver = property_resolver(&mut arena, &[(7, 7)]);
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver.clone());
        frame.locals.set(7, PropValue::Int(5));
        assert_eq!(frame.run().expect("run"), PropValue::Bool(true));

        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver);
        frame.locals.set(7, PropValue::Int(0));
        assert_eq!(frame.run().expect("run"), PropValue::Bool(false));
    }

    #[test]
    fn goto_label_consumes_a_full_name_expression() {
        // UnScrCom.cpp:5036 emits `EX_GotoLabel` then CompileExpr of the
        // label name — an EX_NameConst expression, not a bare compact index.
        let arena = ObjectArena::new();
        let registry = registry();
        let code = [
            USToken::GotoLabel as u8,
            USToken::NameConst as u8,
            5,
            USToken::True as u8,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        assert_eq!(frame.run().expect("run"), PropValue::Bool(true));
    }

    #[test]
    fn struct_compare_consumes_struct_ref_then_both_sides() {
        // UnClass.cpp `case EX_StructCmpEq/Ne`: XFER_OBJECT(UStruct) first,
        // then left and right expressions. The old two-expression-only walk
        // would have read the 0x03 reference byte as a deferrable-gap opcode.
        let mut arena = ObjectArena::new();
        let struct_object = arena.alloc(crate::arena::UObject::default());
        let resolver = BytecodeResolver::from_package_refs(
            Vec::new(),
            vec![struct_object, struct_object, struct_object],
        );
        let registry = registry();
        let code = [
            USToken::StructCmpEq as u8,
            3,
            USToken::IntZero as u8,
            USToken::IntOne as u8,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver);
        // The comparison itself stays a placeholder (constant per token);
        // this pins the operand walk: ref byte, left expr, right expr, then
        // a clean Return — any misread would abort loudly instead.
        assert_eq!(frame.run().expect("run"), PropValue::Bool(true));
    }

    #[test]
    fn rotation_constant_carries_three_components() {
        // UnClass.cpp: `case EX_RotationConst: XFER(INT) x3` (pitch/yaw/
        // roll), exactly like EX_VectorConst's FLOAT x3 — not four words.
        // RotationConst now materializes a real Rotator struct, so the
        // test arena carries the Core.Rotator script struct template.
        let arena = tests::core_struct_arena(&[("Rotator", &["Pitch", "Yaw", "Roll"])]);
        let registry = registry();
        let code = [
            USToken::RotationConst as u8,
            1,
            0,
            0,
            0, //
            2,
            0,
            0,
            0, //
            3,
            0,
            0,
            0, //
            USToken::True as u8,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        let value = frame.run().expect("run");
        assert_eq!(value, PropValue::Bool(true));
        // The constant itself decoded into named Int fields.
        let rot = frame.result.clone();
        let _ = rot;
    }

    #[test]
    fn rotation_constant_decodes_named_int_fields() {
        let arena = tests::core_struct_arena(&[("Rotator", &["Pitch", "Yaw", "Roll"])]);
        let registry = registry();
        let code = [
            USToken::RotationConst as u8,
            1,
            0,
            0,
            0,
            2,
            0,
            0,
            0,
            3,
            0,
            0,
            0,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        assert_eq!(
            frame.run().expect("run"),
            PropValue::Struct {
                struct_name: arena.names.find_index("Rotator").unwrap(),
                fields: vec![
                    (arena.names.find_index("Pitch").unwrap(), PropValue::Int(1)),
                    (arena.names.find_index("Yaw").unwrap(), PropValue::Int(2)),
                    (arena.names.find_index("Roll").unwrap(), PropValue::Int(3)),
                ],
            }
        );
    }

    #[test]
    fn new_defers_after_consuming_four_operand_exprs() {
        // UnClass.cpp `case EX_New` walks parent/name/flags/class exprs;
        // allocation stays deferred but the operands must be consumed.
        let arena = ObjectArena::new();
        let registry = registry();
        let code = [
            USToken::New as u8,
            USToken::IntZero as u8,
            USToken::IntZero as u8,
            USToken::IntZero as u8,
            USToken::IntOne as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        let fail = frame.run().expect_err("New is deferred");
        assert_eq!(fail.reason_code, "vm.token_unsupported");
    }

    #[test]
    fn mover_tick_head_shape_executes_through_native_parms() {
        // Byte-for-byte head of retail Engine.Mover.Tick. Before the
        // BoolVariable fix the native-129 parameter walk recursed into its
        // own EX_EndFunctionParms and aborted the whole run with
        // vm.token_unexpected_end_parms on PrivetDr.Mover0.
        // EX_FinalFunction carries a package object reference: compact bytes
        // 0x5f 0x02 decode to raw export ref 159. The resolver maps that
        // raw reference directly to a native UFunction, not to its name.
        let mut arena = ObjectArena::new();
        for _ in 0..159 {
            arena.alloc(crate::arena::UObject::default());
        }
        let final_function = arena.alloc(crate::arena::UObject {
            class_id: None,
            name_index: 0,
            outer: None,
            flags: 0,
            data: crate::arena::ObjectData::Function(Box::new(crate::arena::FunctionData {
                native_index: 0x81,
                ..Default::default()
            })),
        });
        let mut registry = registry();
        fn native129(
            _frame: &mut Frame<'_>,
            _args: &crate::natives::CallArgs,
        ) -> crate::error::Result<PropValue> {
            Ok(PropValue::Int(1))
        }
        registry
            .register(0x81, 0, native129, "probe.mover.tick")
            .expect("register");
        let code = [
            USToken::FinalFunction as u8,
            0x5f,
            0x02, // compact function ref
            USToken::IntZero as u8,
            USToken::LocalVariable as u8,
            0x55,
            0x37, // multi-byte local ref
            USToken::EndFunctionParms as u8,
            USToken::JumpIfNot as u8,
            0x18,
            0x00, // jump target lands on the Return below
            0x81, // native slot 129
            USToken::BoolVariable as u8,
            USToken::InstanceVariable as u8,
            0x4b,
            0x37, // multi-byte instance ref
            USToken::EndFunctionParms as u8,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let local_name = arena.names.intern("SyntheticLocal");
        let instance_name = arena.names.intern("SyntheticInstance");
        let local = property_object(&mut arena, local_name);
        let instance = property_object(&mut arena, instance_name);
        let resolver = resolver_with_objects(
            &mut arena,
            &[(159, final_function), (3541, local), (3531, instance)],
        );
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver);
        assert_eq!(frame.run().expect("run"), PropValue::Int(1));
    }

    #[test]
    fn final_function_uses_package_object_refs_for_local_and_imported_functions() {
        fn local(
            _frame: &mut Frame<'_>,
            _args: &crate::natives::CallArgs,
        ) -> crate::error::Result<PropValue> {
            Ok(PropValue::Int(11))
        }
        fn imported(
            _frame: &mut Frame<'_>,
            _args: &crate::natives::CallArgs,
        ) -> crate::error::Result<PropValue> {
            Ok(PropValue::Int(22))
        }

        let mut arena = ObjectArena::new();
        let local_name = arena.names.intern("ThisIsNotTheOperand");
        let imported_name = arena.names.intern("NorIsThis");
        let local_fn = arena.alloc(crate::arena::UObject {
            name_index: local_name,
            data: ObjectData::Function(Box::new(crate::arena::FunctionData {
                native_index: 0x80,
                ..Default::default()
            })),
            ..Default::default()
        });
        let imported_fn = arena.alloc(crate::arena::UObject {
            name_index: imported_name,
            data: ObjectData::Function(Box::new(crate::arena::FunctionData {
                native_index: 0x81,
                ..Default::default()
            })),
            ..Default::default()
        });
        let resolver = BytecodeResolver::from_package_refs(vec![Some(imported_fn)], vec![local_fn]);
        let mut registry = registry();
        registry
            .register(0x80, 0, local, "test.local")
            .expect("bind");
        registry
            .register(0x81, 0, imported, "test.imported")
            .expect("bind");
        // Raw `1` selects the local export; compact `0x81` is raw `-1`,
        // selecting the first imported object. Neither byte is a name index.
        let code = [
            USToken::FinalFunction as u8,
            1,
            USToken::EndFunctionParms as u8,
            USToken::FinalFunction as u8,
            0x81,
            USToken::EndFunctionParms as u8,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver);
        assert_eq!(
            frame.run().expect("direct object calls"),
            PropValue::Int(22)
        );

        let wrong_ref = [
            USToken::FinalFunction as u8,
            2,
            USToken::EndFunctionParms as u8,
        ];
        let mut frame = Frame::with_resolver(
            &arena,
            &registry,
            &wrong_ref,
            None,
            BytecodeResolver::from_package_refs(Vec::new(), vec![local_fn]),
        );
        assert_eq!(
            frame
                .run()
                .expect_err("unmapped FinalFunction ref")
                .reason_code,
            "vm.object_ref_unresolved"
        );
    }

    // ---- ScriptRuntime S1: call-frame ABI + expression layer ------------

    /// Regression for the evaluate-and-discard parm walk: a binary native
    /// must receive BOTH operands through the CallArgs vector.
    #[test]
    fn numbered_native_receives_both_operands() {
        let arena = ObjectArena::new();
        let mut registry = registry();
        let body = crate::natives::natives_impl::lookup("Core.Object.Add_IntInt").unwrap();
        registry
            .register(146, 0, body, "Core.Object.Add_IntInt")
            .expect("bind");
        // Add_IntInt(slot 146) <IntConst 7> <IntConst 3>; Return — the
        // fork encodes operator operands INSIDE the native's parm list.
        let code = [
            0x92, // numbered-native slot 146
            USToken::IntConst as u8,
            7,
            0,
            0,
            0,
            USToken::IntConst as u8,
            3,
            0,
            0,
            0,
            USToken::EndFunctionParms as u8,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        assert_eq!(frame.run().expect("run"), PropValue::Int(10));
    }

    #[test]
    fn basecam_locked_vsize_preserves_context_and_lexical_write_target() {
        use crate::arena::{ClassData, PropertyData, UObject};

        let mut arena = core_struct_arena(&[("Vector", &["X", "Y", "Z"])]);
        let vector = arena.find_by_path("Core.Vector").expect("Vector template");
        let vector_name = arena.get(vector).unwrap().name_index;
        let xyz = ["X", "Y", "Z"].map(|field| arena.names.find_index(field).unwrap());
        let vector_value = |values: [f32; 3]| PropValue::Struct {
            struct_name: vector_name,
            fields: xyz
                .into_iter()
                .zip(values)
                .map(|(name, value)| (name, PropValue::Float(value)))
                .collect(),
        };

        let class_name = arena.names.intern("BaseCam");
        let cam_target_name = arena.names.intern("CamTarget");
        let location_name = arena.names.intern("Location");
        let distance_name = arena.names.intern("fLookAtDistance");
        let class = arena.alloc(UObject {
            name_index: class_name,
            data: ObjectData::Class(ClassData::default()),
            ..Default::default()
        });
        let mut property = |name_index, kind| {
            arena.alloc(UObject {
                name_index,
                outer: Some(class),
                data: ObjectData::Property(Box::new(PropertyData {
                    kind,
                    links: Default::default(),
                    array_dim: 1,
                    property_flags: 0,
                    category: 0,
                })),
                ..Default::default()
            })
        };
        let cam_target_property = property(
            cam_target_name,
            PropertyKind::Object {
                class_ref: Some(class),
            },
        );
        let location_property = property(
            location_name,
            PropertyKind::Struct {
                struct_ref: Some(vector),
            },
        );
        let distance_property = property(distance_name, PropertyKind::Float);

        let target = arena.alloc(UObject {
            class_id: Some(class),
            data: ObjectData::Properties({
                let mut store = PropStore::new();
                store.set(location_name, vector_value([3.0, 4.0, 0.0]));
                store
            }),
            ..Default::default()
        });
        let lexical_self = arena.alloc(UObject {
            class_id: Some(class),
            data: ObjectData::Properties({
                let mut store = PropStore::new();
                store.set(cam_target_name, PropValue::Object(Some(target.0 as i32)));
                store.set(location_name, vector_value([0.0, 0.0, 0.0]));
                store.set(distance_name, PropValue::Float(0.0));
                store
            }),
            ..Default::default()
        });
        let resolver = resolver_with_objects(
            &mut arena,
            &[
                (1, cam_target_property),
                (2, location_property),
                (3, distance_property),
            ],
        );
        let code = [
            // BaseCam.uc: CurrentSet.fLookAtDistance =
            //     VSize(CamTarget.Location - Location)
            USToken::Let as u8,
            USToken::InstanceVariable as u8,
            3,
            0xe1, // VSize, a value parameter (P_GET_VECTOR).
            0xd8, // Subtract_VectorVector, itself a nested value expression.
            USToken::Context as u8,
            USToken::InstanceVariable as u8,
            1,
            5,
            0,
            12,
            USToken::InstanceVariable as u8,
            2,
            USToken::InstanceVariable as u8,
            2,
            USToken::EndFunctionParms as u8,
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ];
        let mut registry = NativeRegistry::new();
        registry
            .register(
                216,
                0,
                crate::natives::natives_impl::lookup("Core.Object.Subtract_VectorVector")
                    .expect("subtract native"),
                "Core.Object.Subtract_VectorVector",
            )
            .expect("register subtract");
        registry
            .register(
                225,
                0,
                crate::natives::natives_impl::lookup("Core.Object.VSize").expect("VSize native"),
                "Core.Object.VSize",
            )
            .expect("register VSize");
        registry
            .register(
                223,
                0,
                crate::natives::natives_impl::lookup("Core.Object.AddEqual_VectorVector")
                    .expect("vector add-equal native"),
                "Core.Object.AddEqual_VectorVector",
            )
            .expect("register vector add-equal");

        let mut frame = Frame::with_resolver(
            &arena,
            &registry,
            &code,
            Some(lexical_self),
            resolver.clone(),
        );
        assert_eq!(frame.run().expect("BaseCam Locked expression"), PropValue::Float(5.0));
        assert_eq!(
            frame.take_instance_writes(),
            vec![InstanceWrite {
                object: lexical_self,
                name: distance_name,
                value: PropValue::Float(5.0),
            }]
        );

        // A genuine vector REF parameter may use the same contextual address
        // form. Its write belongs to CamTarget, while the outer activation's
        // lexical Self remains BaseCam.
        let mut contextual_out = vec![
            0xdf, // AddEqual_VectorVector (P_GET_VECTOR_REF first operand).
            USToken::Context as u8,
            USToken::InstanceVariable as u8,
            1,
            5,
            0,
            12,
            USToken::InstanceVariable as u8,
            2,
            USToken::VectorConst as u8,
        ];
        for component in [1.0_f32, 2.0, 3.0] {
            contextual_out.extend_from_slice(&component.to_le_bytes());
        }
        contextual_out.extend_from_slice(&[
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ]);
        let mut out_frame = Frame::with_resolver(
            &arena,
            &registry,
            &contextual_out,
            Some(lexical_self),
            resolver,
        );
        assert_eq!(
            out_frame.run().expect("contextual out lvalue"),
            vector_value([4.0, 6.0, 3.0])
        );
        assert_eq!(
            out_frame.take_instance_writes(),
            vec![InstanceWrite {
                object: target,
                name: location_name,
                value: vector_value([4.0, 6.0, 3.0]),
            }]
        );

        // `0xd8` is still not an address expression. It remains a loud error
        // when malformed bytecode puts it in a genuine P_GET_VECTOR_REF slot.
        let malformed = [0xdd, 0xd8];
        let mut frame = Frame::with_resolver(
            &arena,
            &registry,
            &malformed,
            Some(lexical_self),
            BytecodeResolver::default(),
        );
        let fail = frame.run().expect_err("value expression used as lvalue");
        assert_eq!(fail.reason_code, "vm.lvalue_unsupported");
        assert!(fail.message.contains("opcode 0xd8"));
    }

    /// Minimal class with script functions: `Bar(x)` returns x and
    /// `Foo(x)` returns `Bar(x * 2)` — a named script-to-script call whose
    /// argument must arrive bound to the callee's parameter by name.
    fn script_class_arena() -> (ObjectArena, ObjectId) {
        use crate::arena::{ObjectData, PropertyData, UObject};
        let mut arena = ObjectArena::new();
        let class_name = arena.names.intern("TestClass");
        let class_id = arena.alloc(UObject {
            name_index: class_name,
            data: ObjectData::Class(Default::default()),
            ..Default::default()
        });
        let bar_name = arena.names.intern("Bar");
        let foo_name = arena.names.intern("Foo");
        let x_index = arena.names.intern("x");
        let make_fn = |arena: &mut ObjectArena, name: u32| {
            arena.alloc(UObject {
                name_index: name,
                outer: Some(class_id),
                data: ObjectData::Function(Box::default()),
                ..Default::default()
            })
        };
        let bar_id = make_fn(&mut arena, bar_name);
        let foo_id = make_fn(&mut arena, foo_name);
        let param = |arena: &mut ObjectArena, outer: ObjectId| {
            arena.alloc(UObject {
                name_index: x_index,
                outer: Some(outer),
                data: ObjectData::Property(Box::new(PropertyData {
                    kind: crate::value::PropertyKind::Int,
                    links: Default::default(),
                    array_dim: 1,
                    property_flags: 0,
                    category: 0,
                })),
                ..Default::default()
            })
        };
        let bar_param = param(&mut arena, bar_id);
        let resolver = property_resolver(&mut arena, &[(x_index as i32, x_index)]);
        if let ObjectData::Function(f) = &mut arena.get_mut(bar_id).unwrap().data {
            f.params = vec![bar_param];
            f.code = vec![
                USToken::LocalVariable as u8,
                x_index as u8,
                USToken::Return as u8,
                USToken::Nothing as u8,
            ];
            f.resolver = resolver.clone();
        }
        let foo_param = param(&mut arena, foo_id);
        if let ObjectData::Function(f) = &mut arena.get_mut(foo_id).unwrap().data {
            f.params = vec![foo_param];
            f.code = vec![
                // VirtualFunction Bar( LocalVariable x * IntConstByte 2 )
                USToken::VirtualFunction as u8,
                bar_name as u8,
                146, // Multiply_IntInt slot — operands FOLLOW the opcode
                USToken::LocalVariable as u8,
                x_index as u8,
                USToken::IntConstByte as u8,
                2,
                USToken::EndFunctionParms as u8, // inner (operator)
                USToken::EndFunctionParms as u8, // outer (Bar call)
                USToken::Return as u8,
                USToken::Nothing as u8,
            ];
            f.resolver = resolver;
        }
        (arena, class_id)
    }

    /// A bare TestClass instance object.
    fn instance_of(arena: &mut ObjectArena, class_id: ObjectId) -> ObjectId {
        arena.alloc(crate::arena::UObject {
            class_id: Some(class_id),
            data: crate::arena::ObjectData::Properties(crate::props::PropStore::new()),
            ..Default::default()
        })
    }

    #[test]
    fn named_script_call_binds_arguments_by_name() {
        let (arena, class_id) = script_class_arena();
        let registry = registry_with_core_ops();
        let foo_id = arena.find_child(class_id, "Foo").expect("Foo");
        let (code, resolver) = match &arena.get(foo_id).unwrap().data {
            crate::arena::ObjectData::Function(f) => (f.code.clone(), f.resolver.clone()),
            _ => unreachable!(),
        };
        let mut arena = arena;
        let self_id = instance_of(&mut arena, class_id);
        let mut frame = Frame::with_resolver(&arena, &registry, &code, Some(self_id), resolver);
        frame
            .locals
            .set(arena.names.find_index("x").unwrap(), PropValue::Int(21));
        assert_eq!(frame.run().expect("run"), PropValue::Int(42));
    }

    #[test]
    fn unresolvable_named_call_defers_loudly_after_operand_walk() {
        let arena = ObjectArena::new();
        let registry = registry();
        // VirtualFunction Missing( IntOne ) — no self, no functions at all.
        let code = [
            USToken::VirtualFunction as u8,
            3, // name index 3 (absent from the pool)
            USToken::IntOne as u8,
            USToken::EndFunctionParms as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        let fail = frame.run().expect_err("unresolved call must fail loudly");
        assert_eq!(fail.reason_code, "vm.function_unresolved");
    }

    #[test]
    fn recursion_is_capped_with_a_loud_defer() {
        use crate::arena::ObjectData;
        let (mut arena, class_id) = script_class_arena();
        let foo_name = arena.names.find_index("Foo").unwrap();
        let x_index = arena.names.find_index("x").unwrap();
        // Bar recurses into Foo.
        let bar_id = arena.find_child(class_id, "Bar").unwrap();
        if let ObjectData::Function(f) = &mut arena.get_mut(bar_id).unwrap().data {
            f.code = vec![
                USToken::VirtualFunction as u8,
                foo_name as u8,
                USToken::LocalVariable as u8,
                x_index as u8,
                USToken::EndFunctionParms as u8,
                USToken::Return as u8,
                USToken::Nothing as u8,
            ];
        }
        let foo_id = arena.find_child(class_id, "Foo").unwrap();
        let (code, resolver) = match &arena.get(foo_id).unwrap().data {
            ObjectData::Function(f) => (f.code.clone(), f.resolver.clone()),
            _ => unreachable!(),
        };
        let self_id = instance_of(&mut arena, class_id);
        let registry = registry_with_core_ops();
        let mut frame = Frame::with_resolver(&arena, &registry, &code, Some(self_id), resolver);
        let fail = frame.run().expect_err("recursion must hit the depth cap");
        assert_eq!(fail.reason_code, "vm.call_depth_exceeded");
    }

    #[test]
    fn instance_writes_queue_for_caller_application() {
        let (mut arena, class_id) = script_class_arena();
        let health_index = arena.names.intern("Health");
        let self_id = instance_of(&mut arena, class_id);
        let registry = registry();
        // Let <InstanceVariable Health> <IntConst 5>; Return.
        let code = [
            USToken::Let as u8,
            USToken::InstanceVariable as u8,
            health_index as u8,
            USToken::IntConst as u8,
            5,
            0,
            0,
            0,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let resolver = property_resolver(&mut arena, &[(health_index as i32, health_index)]);
        let mut frame = Frame::with_resolver(&arena, &registry, &code, Some(self_id), resolver);
        frame.run().expect("run");
        // The write must NOT be in locals; it queues for the caller.
        assert_eq!(frame.locals.get(health_index), None);
        assert_eq!(
            frame.take_instance_writes(),
            vec![InstanceWrite {
                object: self_id,
                name: health_index,
                value: PropValue::Int(5),
            }]
        );
    }
    #[test]
    fn compound_natives_write_local_and_instance_lvalues() {
        let (mut arena, class_id) = script_class_arena();
        let local_name = arena.names.intern("LocalFloat");
        let instance_name = arena.names.intern("InstanceFloat");
        let self_id = instance_of(&mut arena, class_id);
        if let ObjectData::Properties(store) = &mut arena.get_mut(self_id).unwrap().data {
            store.set(instance_name, PropValue::Float(4.0));
        }
        let resolver = property_resolver(&mut arena, &[(1, local_name), (2, instance_name)]);
        let mut code = vec![
            0xb8,
            USToken::LocalVariable as u8,
            1,
            USToken::FloatConst as u8,
        ];
        code.extend_from_slice(&2.5f32.to_le_bytes());
        code.extend_from_slice(&[
            USToken::EndFunctionParms as u8,
            0xb8,
            USToken::InstanceVariable as u8,
            2,
            USToken::FloatConst as u8,
        ]);
        code.extend_from_slice(&1.25f32.to_le_bytes());
        code.extend_from_slice(&[USToken::EndFunctionParms as u8, USToken::Stop as u8]);
        let registry = registry_with_core_ops();
        let mut frame = Frame::with_resolver(&arena, &registry, &code, Some(self_id), resolver);
        frame.locals.set(local_name, PropValue::Float(1.5));
        assert_eq!(frame.run().expect("compound run"), PropValue::Float(5.25));
        assert_eq!(frame.locals.get(local_name), Some(&PropValue::Float(4.0)));
        assert_eq!(
            frame.take_instance_writes(),
            vec![InstanceWrite {
                object: self_id,
                name: instance_name,
                value: PropValue::Float(5.25),
            }]
        );
    }
    #[test]
    fn compound_native_reread_preserves_nonzero_class_default() {
        use crate::arena::{PropertyData, UObject};
        let (mut arena, class_id) = script_class_arena();
        let name = arena.names.intern("DefaultFloat");
        let property = arena.alloc(UObject {
            name_index: name,
            outer: Some(class_id),
            data: ObjectData::Property(Box::new(PropertyData {
                links: Default::default(),
                kind: PropertyKind::Float,
                array_dim: 1,
                property_flags: 0,
                category: 0,
            })),
            ..Default::default()
        });
        let default_object = arena.alloc(UObject {
            class_id: Some(class_id),
            data: ObjectData::Properties({
                let mut store = PropStore::new();
                store.set(name, PropValue::Float(5.0));
                store
            }),
            ..Default::default()
        });
        let ObjectData::Class(class) = &mut arena.get_mut(class_id).unwrap().data else {
            unreachable!()
        };
        class.default_object = Some(default_object);
        let actor = instance_of(&mut arena, class_id);
        let resolver = resolver_with_objects(&mut arena, &[(1, property)]);
        let mut code = vec![
            0xb8,
            USToken::InstanceVariable as u8,
            1,
            USToken::FloatConst as u8,
        ];
        code.extend_from_slice(&1.0f32.to_le_bytes());
        code.extend_from_slice(&[USToken::EndFunctionParms as u8, USToken::Stop as u8]);
        let registry = registry_with_core_ops();
        let mut frame = Frame::with_resolver(&arena, &registry, &code, Some(actor), resolver);
        assert_eq!(
            frame.run().expect("default compound"),
            PropValue::Float(6.0)
        );
        assert_eq!(
            frame.take_instance_writes(),
            vec![InstanceWrite {
                object: actor,
                name,
                value: PropValue::Float(6.0),
            }]
        );
    }

    #[test]
    fn compound_native_rereads_lvalue_after_aliasing_rhs_mutation() {
        let mut arena = ObjectArena::new();
        let name = arena.names.intern("Value");
        let resolver = property_resolver(&mut arena, &[(1, name)]);
        // Value += (Value += 1). Inner assignment changes Value from 1 to 2
        // and returns 2; outer P_GET_FLOAT_REF must then re-read 2, yielding 4.
        let mut code = vec![
            0xb8,
            USToken::LocalVariable as u8,
            1,
            0xb8,
            USToken::LocalVariable as u8,
            1,
            USToken::FloatConst as u8,
        ];
        code.extend_from_slice(&1.0f32.to_le_bytes());
        code.extend_from_slice(&[
            USToken::EndFunctionParms as u8,
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ]);
        let registry = registry_with_core_ops();
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver);
        frame.locals.set(name, PropValue::Float(1.0));
        assert_eq!(
            frame.run().expect("aliasing compound"),
            PropValue::Float(4.0)
        );
        assert_eq!(frame.locals.get(name), Some(&PropValue::Float(4.0)));
    }
    #[test]
    fn multiply_equal_rereads_lvalue_after_aliasing_rhs_mutation() {
        let mut arena = ObjectArena::new();
        let name = arena.names.intern("Value");
        let resolver = property_resolver(&mut arena, &[(1, name)]);
        // Value *= (Value += 1): inner changes 2 to 3 and returns 3;
        // MultiplyEqual must re-read 3, then assign 3*3.
        let mut code = vec![
            0xb6,
            USToken::LocalVariable as u8,
            1,
            0xb8,
            USToken::LocalVariable as u8,
            1,
            USToken::FloatConst as u8,
        ];
        code.extend_from_slice(&1.0f32.to_le_bytes());
        code.extend_from_slice(&[
            USToken::EndFunctionParms as u8,
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ]);
        let registry = registry_with_core_ops();
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver);
        frame.locals.set(name, PropValue::Float(2.0));
        assert_eq!(
            frame.run().expect("aliasing multiply-equal"),
            PropValue::Float(9.0)
        );
        assert_eq!(frame.locals.get(name), Some(&PropValue::Float(9.0)));
    }
    #[test]
    fn add_equal_int_rereads_after_postincrement_rhs() {
        let mut arena = ObjectArena::new();
        let name = arena.names.intern("Value");
        let resolver = property_resolver(&mut arena, &[(1, name)]);
        // Value += Value++: postfix returns 2 but writes 3 first, so the
        // address-bound outer compound re-reads 3 and assigns 5.
        let code = [
            0xa1,
            USToken::LocalVariable as u8,
            1,
            0xa5,
            USToken::LocalVariable as u8,
            1,
            USToken::EndFunctionParms as u8,
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ];
        let registry = registry_with_core_ops();
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver);
        frame.locals.set(name, PropValue::Int(2));
        assert_eq!(
            frame.run().expect("aliasing int add-equal"),
            PropValue::Int(5)
        );
        assert_eq!(frame.locals.get(name), Some(&PropValue::Int(5)));
    }

    #[test]
    fn pre_and_post_increment_have_distinct_return_values() {
        let mut arena = ObjectArena::new();
        let name = arena.names.intern("Counter");
        let resolver = property_resolver(&mut arena, &[(1, name)]);
        let pre_code = [
            0xa3,
            USToken::LocalVariable as u8,
            1,
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ];
        let post_code = [
            0xa5,
            USToken::LocalVariable as u8,
            1,
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ];
        let registry = registry_with_core_ops();
        let mut pre = Frame::with_resolver(&arena, &registry, &pre_code, None, resolver.clone());
        pre.locals.set(name, PropValue::Int(41));
        assert_eq!(pre.run().expect("preincrement"), PropValue::Int(42));
        assert_eq!(pre.locals.get(name), Some(&PropValue::Int(42)));

        let mut post = Frame::with_resolver(&arena, &registry, &post_code, None, resolver);
        post.locals.set(name, PropValue::Int(41));
        assert_eq!(post.run().expect("postincrement"), PropValue::Int(41));
        assert_eq!(post.locals.get(name), Some(&PropValue::Int(42)));
    }
    #[test]
    fn pre_and_post_decrement_have_distinct_return_values() {
        let mut arena = ObjectArena::new();
        let name = arena.names.intern("Counter");
        let resolver = property_resolver(&mut arena, &[(1, name)]);
        let pre_code = [
            0xa4,
            USToken::LocalVariable as u8,
            1,
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ];
        let post_code = [
            0xa6,
            USToken::LocalVariable as u8,
            1,
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ];
        let registry = registry_with_core_ops();
        let mut pre = Frame::with_resolver(&arena, &registry, &pre_code, None, resolver.clone());
        pre.locals.set(name, PropValue::Int(41));
        assert_eq!(pre.run().expect("predecrement"), PropValue::Int(40));
        assert_eq!(pre.locals.get(name), Some(&PropValue::Int(40)));

        let mut post = Frame::with_resolver(&arena, &registry, &post_code, None, resolver);
        post.locals.set(name, PropValue::Int(41));
        assert_eq!(post.run().expect("postdecrement"), PropValue::Int(41));
        assert_eq!(post.locals.get(name), Some(&PropValue::Int(40)));
    }

    #[test]
    fn compound_native_rebuilds_static_array_element() {
        let mut arena = ObjectArena::new();
        let array_name = arena.names.intern("Values");
        let index_name = arena.names.intern("Index");
        let resolver = property_resolver(&mut arena, &[(1, array_name), (2, index_name)]);
        let mut code = vec![
            0xb8,
            USToken::ArrayElement as u8,
            USToken::LocalVariable as u8,
            2,
            USToken::LocalVariable as u8,
            1,
            USToken::FloatConst as u8,
        ];
        code.extend_from_slice(&0.5f32.to_le_bytes());
        code.extend_from_slice(&[USToken::EndFunctionParms as u8, USToken::Stop as u8]);
        let registry = registry_with_core_ops();
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver);
        frame.locals.set(
            array_name,
            PropValue::FixedArray(vec![PropValue::Float(1.0), PropValue::Float(2.0)]),
        );
        frame.locals.set(index_name, PropValue::Int(1));
        assert_eq!(frame.run().expect("array compound"), PropValue::Float(2.5));
        assert_eq!(
            frame.locals.get(array_name),
            Some(&PropValue::FixedArray(vec![
                PropValue::Float(1.0),
                PropValue::Float(2.5)
            ]))
        );
    }
    #[test]
    fn compound_native_normalizes_instance_and_default_static_array_scalars_on_reread() {
        use crate::arena::{ClassData, PropertyData, UObject};
        let mut arena = ObjectArena::new();
        let class_name = arena.names.intern("ArrayOwner");
        let values_name = arena.names.intern("Values");
        let class_id = arena.alloc(UObject {
            name_index: class_name,
            data: ObjectData::Class(ClassData::default()),
            ..Default::default()
        });
        let property = arena.alloc(UObject {
            name_index: values_name,
            outer: Some(class_id),
            data: ObjectData::Property(Box::new(PropertyData {
                links: Default::default(),
                kind: PropertyKind::Float,
                array_dim: 3,
                property_flags: 0,
                category: 0,
            })),
            ..Default::default()
        });
        let default_object = arena.alloc(UObject {
            class_id: Some(class_id),
            outer: Some(class_id),
            data: ObjectData::Properties({
                let mut store = PropStore::new();
                store.set(values_name, PropValue::Float(5.0));
                store
            }),
            ..Default::default()
        });
        let ObjectData::Class(class) = &mut arena.get_mut(class_id).unwrap().data else {
            unreachable!()
        };
        class.default_object = Some(default_object);
        let actor = arena.alloc(UObject {
            class_id: Some(class_id),
            data: ObjectData::Properties({
                let mut store = PropStore::new();
                store.set(values_name, PropValue::Float(7.0));
                store
            }),
            ..Default::default()
        });
        let resolver = resolver_with_objects(&mut arena, &[(1, property)]);
        let mut instance_code = vec![
            0xb8,
            USToken::ArrayElement as u8,
            USToken::IntZero as u8,
            USToken::InstanceVariable as u8,
            1,
            USToken::FloatConst as u8,
        ];
        instance_code.extend_from_slice(&1.0f32.to_le_bytes());
        instance_code.extend_from_slice(&[
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ]);
        let mut default_code = instance_code.clone();
        default_code[3] = USToken::DefaultVariable as u8;
        let registry = registry_with_core_ops();

        let mut instance =
            Frame::with_resolver(&arena, &registry, &instance_code, Some(actor), resolver.clone());
        assert_eq!(instance.run().expect("instance static array"), PropValue::Float(8.0));
        assert_eq!(
            instance.take_instance_writes(),
            vec![InstanceWrite {
                object: actor,
                name: values_name,
                value: PropValue::FixedArray(vec![
                    PropValue::Float(8.0),
                    PropValue::Float(0.0),
                    PropValue::Float(0.0),
                ]),
            }]
        );

        let mut defaults =
            Frame::with_resolver(&arena, &registry, &default_code, Some(actor), resolver);
        assert_eq!(defaults.run().expect("default static array"), PropValue::Float(6.0));
        assert_eq!(
            defaults.take_instance_writes(),
            vec![InstanceWrite {
                object: default_object,
                name: values_name,
                value: PropValue::FixedArray(vec![
                    PropValue::Float(6.0),
                    PropValue::Float(0.0),
                    PropValue::Float(0.0),
                ]),
            }]
        );
    }


    #[test]
    fn compound_native_rebuilds_struct_field() {
        let mut arena = core_struct_arena(&[("Vector", &["X", "Y", "Z"])]);
        let vector_name = arena.names.find_index("Vector").unwrap();
        let x_name = arena.names.find_index("X").unwrap();
        let resolver = property_resolver(&mut arena, &[(1, vector_name), (2, x_name)]);
        let mut code = vec![
            0xb8,
            USToken::StructMember as u8,
            2,
            USToken::LocalVariable as u8,
            1,
            USToken::FloatConst as u8,
        ];
        code.extend_from_slice(&3.0f32.to_le_bytes());
        code.extend_from_slice(&[USToken::EndFunctionParms as u8, USToken::Stop as u8]);
        let registry = registry_with_core_ops();
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver);
        frame.locals.set(
            vector_name,
            PropValue::Struct {
                struct_name: vector_name,
                fields: vec![
                    (x_name, PropValue::Float(2.0)),
                    (arena.names.find_index("Y").unwrap(), PropValue::Float(0.0)),
                    (arena.names.find_index("Z").unwrap(), PropValue::Float(0.0)),
                ],
            },
        );
        assert_eq!(frame.run().expect("struct compound"), PropValue::Float(5.0));
        let PropValue::Struct { fields, .. } = frame.locals.get(vector_name).unwrap() else {
            panic!("vector local changed type");
        };
        assert_eq!(
            fields.iter().find(|(name, _)| *name == x_name).unwrap().1,
            PropValue::Float(5.0)
        );
    }

    #[test]
    fn imported_struct_field_lvalue_updates_its_instance_container() {
        let mut arena = ObjectArena::new();
        let actor_name = arena.names.intern("Actor");
        let color_name = arena.names.intern("SparkleColor");
        let struct_name = arena.names.intern("ColorParams");
        let base_name = arena.names.intern("Base");
        let unrelated = arena.alloc(crate::arena::UObject {
            name_index: base_name,
            data: ObjectData::Properties(crate::props::PropStore::new()),
            ..Default::default()
        });
        let color_property = property_object(&mut arena, color_name);
        let base_property = property_object(&mut arena, base_name);
        let actor = arena.alloc(crate::arena::UObject {
            name_index: actor_name,
            data: ObjectData::Properties(crate::props::PropStore::new()),
            ..Default::default()
        });
        arena.get_mut(actor).unwrap().properties_mut().unwrap().set(
            color_name,
            PropValue::Struct {
                struct_name,
                fields: vec![(base_name, PropValue::Float(2.0))],
            },
        );

        // HGame's forward import is #129: compact -129 is c1 02. Keep an
        // unrelated arena object named Base in the same arena to prove the
        // package resolver selects the imported UProperty identity.
        let mut imports = vec![None; 129];
        imports[0] = Some(unrelated);
        imports[128] = Some(base_property);
        let resolver = BytecodeResolver::from_package_refs_and_names_for_package(
            "Synthetic",
            imports,
            vec![color_property],
            Vec::new(),
        );
        let mut code = vec![
            0xb8,
            USToken::StructMember as u8,
            0xc1,
            0x02,
            USToken::InstanceVariable as u8,
            1,
            USToken::FloatConst as u8,
        ];
        code.extend_from_slice(&3.0f32.to_le_bytes());
        code.extend_from_slice(&[USToken::EndFunctionParms as u8, USToken::Stop as u8]);

        let registry = registry_with_core_ops();
        let mut frame =
            Frame::with_resolver(&arena, &registry, &code, Some(actor), resolver.clone());
        assert_eq!(frame.run().expect("struct field +="), PropValue::Float(5.0));
        assert_eq!(
            frame.take_instance_writes(),
            vec![InstanceWrite {
                object: actor,
                name: color_name,
                value: PropValue::Struct {
                    struct_name,
                    fields: vec![(base_name, PropValue::Float(5.0))],
                },
            }]
        );
        assert!(
            arena
                .get(unrelated)
                .unwrap()
                .properties()
                .unwrap()
                .is_empty(),
            "same-named unrelated arena object must not receive the write"
        );

        let invalid_code = [
            0xb8,
            USToken::StructMember as u8,
            0x81,
            USToken::InstanceVariable as u8,
            1,
            USToken::FloatConst as u8,
            0,
            0,
            0,
            0,
            USToken::EndFunctionParms as u8,
            USToken::Stop as u8,
        ];
        let mut invalid =
            Frame::with_resolver(&arena, &registry, &invalid_code, Some(actor), resolver);
        let fail = invalid
            .run()
            .expect_err("non-property import must stay invalid");
        assert_eq!(fail.reason_code, "vm.object_ref_type");
        assert!(
            fail.message
                .contains("raw ref -1 in package Synthetic at pc 2")
        );
    }

    #[test]
    fn struct_member_extracts_named_field() {
        let mut arena = core_struct_arena(&[("Vector", &["X", "Y", "Z"])]);
        let registry = registry();
        let y_index = arena.names.find_index("Y").unwrap();
        // StructMember Y <VectorConst(1.0, 2.5, -4.0)>; Return.
        let one = 1.0f32.to_le_bytes();
        let twohalf = 2.5f32.to_le_bytes();
        let negfour = (-4.0f32).to_le_bytes();
        let mut code = vec![
            USToken::StructMember as u8,
            y_index as u8,
            USToken::VectorConst as u8,
        ];
        code.extend_from_slice(&one);
        code.extend_from_slice(&twohalf);
        code.extend_from_slice(&negfour);
        code.extend_from_slice(&[USToken::Return as u8, USToken::Nothing as u8]);
        let resolver = property_resolver(&mut arena, &[(y_index as i32, y_index)]);
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver);
        assert_eq!(frame.run().expect("run"), PropValue::Float(2.5));
    }

    #[test]
    fn sparse_cam_settings_materializes_declared_float_and_rejects_foreign_field() {
        use crate::arena::{PropertyData, StructData, UObject};

        let mut arena = ObjectArena::new();
        let root = arena.root_for("HGame");
        let cam_name = arena.names.intern("CamSettings");
        let offset_name = arena.names.intern("vLookAtOffset");
        let distance_name = arena.names.intern("fLookAtDistance");
        let invalid_name = arena.names.intern("NotACamSetting");
        let local_name = arena.names.intern("SparseCamSettings");
        let cam_struct = arena.alloc(UObject {
            name_index: cam_name,
            outer: Some(root),
            data: ObjectData::ScriptStruct(StructData::default()),
            ..Default::default()
        });
        let offset_property = arena.alloc(UObject {
            name_index: offset_name,
            outer: Some(cam_struct),
            data: ObjectData::Property(Box::new(PropertyData {
                links: Default::default(),
                kind: PropertyKind::Struct { struct_ref: None },
                array_dim: 1,
                property_flags: 0,
                category: 0,
            })),
            ..Default::default()
        });
        let distance_property = arena.alloc(UObject {
            name_index: distance_name,
            outer: Some(cam_struct),
            data: ObjectData::Property(Box::new(PropertyData {
                links: Default::default(),
                kind: PropertyKind::Float,
                array_dim: 1,
                property_flags: 0,
                category: 0,
            })),
            ..Default::default()
        });
        let other_name = arena.names.intern("OtherSettings");
        let other_struct = arena.alloc(UObject {
            name_index: other_name,
            outer: Some(root),
            data: ObjectData::ScriptStruct(StructData::default()),
            ..Default::default()
        });
        let invalid_property = arena.alloc(UObject {
            name_index: invalid_name,
            outer: Some(other_struct),
            data: ObjectData::Property(Box::new(PropertyData {
                links: Default::default(),
                kind: PropertyKind::Float,
                array_dim: 1,
                property_flags: 0,
                category: 0,
            })),
            ..Default::default()
        });
        if let ObjectData::ScriptStruct(data) = &mut arena.get_mut(cam_struct).unwrap().data {
            data.children = vec![offset_property, distance_property];
        }
        if let ObjectData::ScriptStruct(data) = &mut arena.get_mut(other_struct).unwrap().data {
            data.children = vec![invalid_property];
        }
        let local_property = arena.alloc(UObject {
            name_index: local_name,
            data: ObjectData::Property(Box::new(PropertyData {
                links: Default::default(),
                kind: PropertyKind::Struct {
                    struct_ref: Some(cam_struct),
                },
                array_dim: 1,
                property_flags: 0,
                category: 0,
            })),
            ..Default::default()
        });
        let resolver = resolver_with_objects(
            &mut arena,
            &[
                (1, local_property),
                (2, distance_property),
                (3, invalid_property),
            ],
        );
        let sparse = PropValue::Struct {
            struct_name: cam_name,
            fields: vec![(
                offset_name,
                PropValue::Struct {
                    struct_name: arena.names.intern("Vector"),
                    fields: Vec::new(),
                },
            )],
        };
        let registry = registry();
        let code = [
            USToken::StructMember as u8,
            2,
            USToken::LocalVariable as u8,
            1,
            USToken::Stop as u8,
        ];
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver.clone());
        frame.locals.set(local_name, sparse.clone());
        assert_eq!(
            frame.run().expect("declared sparse member"),
            PropValue::Float(0.0)
        );

        let invalid_code = [
            USToken::StructMember as u8,
            3,
            USToken::LocalVariable as u8,
            1,
            USToken::Stop as u8,
        ];
        let mut invalid = Frame::with_resolver(&arena, &registry, &invalid_code, None, resolver);
        invalid.locals.set(local_name, sparse);
        let fail = invalid
            .run()
            .expect_err("foreign field must remain invalid");
        assert_eq!(fail.reason_code, "vm.token_unsupported");
        assert!(fail.message.contains("NotACamSetting"));
    }

    #[test]
    fn context_evaluates_members_against_the_target_object() {
        use crate::arena::ObjectData;
        let (mut arena, class_id) = script_class_arena();
        let health_index = arena.names.intern("Health");
        let viewer = instance_of(&mut arena, class_id);
        // A second instance carrying Health = -7 on its own store.
        let target_name = arena.names.intern("Target");
        let target = arena.alloc(crate::arena::UObject {
            name_index: target_name,
            class_id: Some(class_id),
            data: ObjectData::Properties(crate::props::PropStore::new()),
            ..Default::default()
        });
        if let ObjectData::Properties(store) = &mut arena.get_mut(target).unwrap().data {
            store.set(health_index, PropValue::Int(-7));
        }
        let registry = registry();
        // Context <ObjectConst target> <offset=6> <fill=0> member;
        //   member = InstanceVariable Health ; Return Nothing
        let code = [
            USToken::Context as u8,
            USToken::ObjectConst as u8,
            1, // package object reference to the target
            6,
            0, // null-jump offset → the Return below
            0, // zero-fill size
            USToken::InstanceVariable as u8,
            health_index as u8,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let health_property = property_object(&mut arena, health_index);
        let resolver = resolver_with_objects(
            &mut arena,
            &[(1, target), (health_index as i32, health_property)],
        );
        let mut frame = Frame::with_resolver(&arena, &registry, &code, Some(viewer), resolver);
        assert_eq!(frame.run().expect("run"), PropValue::Int(-7));
    }

    #[test]
    fn array_element_indexes_fixed_arrays_and_defers_out_of_bounds() {
        let mut arena = ObjectArena::new();
        let registry = registry();
        // ArrayElement <Local 9 (index)> <Local 8 (array)>.
        let code = [
            USToken::ArrayElement as u8,
            USToken::LocalVariable as u8,
            9,
            USToken::LocalVariable as u8,
            8,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let resolver = property_resolver(&mut arena, &[(9, 9), (8, 8)]);
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver.clone());
        frame.locals.set(
            8,
            PropValue::FixedArray(vec![PropValue::Int(10), PropValue::Int(20)]),
        );

        frame.locals.set(9, PropValue::Int(1));
        assert_eq!(frame.run().expect("run"), PropValue::Int(20));

        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver);
        frame
            .locals
            .set(8, PropValue::FixedArray(vec![PropValue::Int(10)]));
        frame.locals.set(9, PropValue::Int(5));
        let fail = frame.run().expect_err("OOB index must defer loudly");
        assert_eq!(fail.reason_code, "vm.array_out_of_bounds");
    }
    #[test]
    fn static_array_defaults_follow_declared_object_vector_and_int_kinds() {
        use crate::arena::{ObjectData, PropertyData, UObject};

        let mut arena = core_struct_arena(&[("Vector", &["X", "Y", "Z"])]);
        let vector = arena.find_by_path("Core.Vector").expect("Vector template");
        let vector_children = match &arena.get(vector).expect("Vector").data {
            ObjectData::ScriptStruct(data) => data.children.clone(),
            _ => panic!("Vector must be a script struct"),
        };
        for field in vector_children {
            let ObjectData::Property(data) = &mut arena.get_mut(field).expect("field").data else {
                panic!("Vector child must be a property");
            };
            data.kind = PropertyKind::Float;
        }

        let mut property = |name: &str, kind: PropertyKind, array_dim: i32| {
            let name_index = arena.names.intern(name);
            arena.alloc(UObject {
                name_index,
                data: ObjectData::Property(Box::new(PropertyData {
                    links: Default::default(),
                    kind,
                    array_dim,
                    property_flags: 0,
                    category: 0,
                })),
                ..Default::default()
            })
        };
        let objects = property("Objects", PropertyKind::Object { class_ref: None }, 3);
        let vectors = property(
            "Vectors",
            PropertyKind::Struct {
                struct_ref: Some(vector),
            },
            2,
        );
        let ints = property("Ints", PropertyKind::Int, 4);

        let index = property("Index", PropertyKind::Int, 1);
        assert_eq!(
            arena
                .default_for_property(objects)
                .expect("object defaults"),
            PropValue::FixedArray(vec![PropValue::Object(None); 3])
        );
        assert_eq!(
            arena.default_for_property(ints).expect("int defaults"),
            PropValue::FixedArray(vec![PropValue::Int(0); 4])
        );
        let PropValue::FixedArray(values) = arena
            .default_for_property(vectors)
            .expect("vector defaults")
        else {
            panic!("vector static property must materialize a fixed array");
        };
        assert_eq!(values.len(), 2);
        for value in values {
            let PropValue::Struct {
                struct_name,
                fields,
            } = value
            else {
                panic!("vector element must retain struct type");
            };
            assert_eq!(struct_name, arena.get(vector).expect("Vector").name_index);
            assert_eq!(fields.len(), 3);
            assert!(
                fields
                    .iter()
                    .all(|(_, value)| *value == PropValue::Float(0.0))
            );
        }
        // Real EX_ArrayElement bytecode over a missing local static array:
        // the property operand supplies both element type and ArrayDim.
        let resolver = resolver_with_objects(&mut arena, &[(1, ints), (2, index)]);
        let code = [
            USToken::ArrayElement as u8,
            USToken::LocalVariable as u8,
            2,
            USToken::LocalVariable as u8,
            1,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];

        let registry = registry();
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver);
        let index_name = arena.get(index).expect("index property").name_index;
        frame.locals.set(index_name, PropValue::Int(3));
        assert_eq!(frame.run().expect("static ArrayElement"), PropValue::Int(0));
    }
    #[test]
    fn null_context_materializes_member_property_typed_vector_zero() {
        use crate::arena::{ObjectData, PropertyData, UObject};

        let mut arena = core_struct_arena(&[("Vector", &["X", "Y", "Z"])]);
        let vector = arena
            .objects_iter()
            .find(|(_, object)| {
                matches!(object.data, ObjectData::ScriptStruct(_))
                    && arena.names.text(object.name_index) == Some("Vector")
            })
            .map(|(id, _)| id)
            .expect("Vector");
        let children = match &arena.get(vector).unwrap().data {
            ObjectData::ScriptStruct(data) => data.children.clone(),
            _ => unreachable!(),
        };
        for child in &children {
            let ObjectData::Property(data) = &mut arena.get_mut(*child).unwrap().data else {
                panic!("Vector child");
            };
            data.kind = PropertyKind::Float;
        }
        let x = children
            .iter()
            .copied()
            .find(|id| arena.names.text(arena.get(*id).unwrap().name_index) == Some("X"))
            .unwrap();
        let velocity_name = arena.names.intern("Velocity");
        let velocity = arena.alloc(UObject {
            name_index: velocity_name,
            data: ObjectData::Property(Box::new(PropertyData {
                kind: PropertyKind::Struct {
                    struct_ref: Some(vector),
                },
                links: Default::default(),
                array_dim: 1,
                property_flags: 0,
                category: 0,
            })),
            ..Default::default()
        });
        let resolver = resolver_with_objects(&mut arena, &[(1, x), (2, velocity)]);
        // X ( None.Velocity ): EX_Context carries a two-byte member expression
        // and a 12-byte zero-fill size. The serialized Velocity UProperty,
        // rather than an Int fallback, supplies the typed Vector zero.
        let code = [
            USToken::StructMember as u8,
            1,
            USToken::Context as u8,
            USToken::IntZero as u8,
            5,
            0,
            12,
            USToken::InstanceVariable as u8,
            2,
            USToken::Stop as u8,
        ];
        let registry = registry();
        let mut frame = Frame::with_resolver(&arena, &registry, &code, None, resolver);
        assert_eq!(
            frame.run().expect("typed null context"),
            PropValue::Float(0.0)
        );
        drop(frame);

        let scalar_name = arena.names.intern("Scalar");
        let scalar_property = arena.alloc(UObject {
            name_index: scalar_name,
            data: ObjectData::Property(Box::new(PropertyData {
                kind: PropertyKind::Int,
                links: Default::default(),
                array_dim: 1,
                property_flags: 0,
                category: 0,
            })),
            ..Default::default()
        });
        let scalar_resolver = resolver_with_objects(&mut arena, &[(1, x), (2, scalar_property)]);
        let scalar_code = [
            USToken::StructMember as u8,
            1,
            USToken::LocalVariable as u8,
            2,
            USToken::Stop as u8,
        ];
        let mut scalar =
            Frame::with_resolver(&arena, &registry, &scalar_code, None, scalar_resolver);
        scalar.locals.set(scalar_name, PropValue::Int(0));
        let fail = scalar
            .run()
            .expect_err("scalar member access must stay loud");
        assert_eq!(fail.reason_code, "vm.token_unsupported");
    }

    #[test]
    fn index_none_localize_dispatches_result_effect_instead_of_parameter_bytecode() {
        use crate::arena::{ObjectData, PropertyData, UObject};

        let (mut arena, class_id) = script_class_arena();
        let function_name = arena.names.intern("Localize");
        let function = arena.alloc(UObject {
            name_index: function_name,
            outer: Some(class_id),
            data: ObjectData::Function(Box::default()),
            ..Default::default()
        });
        let mut params = Vec::new();
        for name in ["SectionName", "KeyName", "PackageName"] {
            let name_index = arena.names.intern(name);
            params.push(arena.alloc(UObject {
                name_index,
                outer: Some(function),
                data: ObjectData::Property(Box::new(PropertyData {
                    kind: PropertyKind::Str,
                    links: Default::default(),
                    array_dim: 1,
                    property_flags: 0,
                    category: 0,
                })),
                ..Default::default()
            }));
        }
        if let ObjectData::Function(data) = &mut arena.get_mut(function).unwrap().data {
            data.params = params;
            data.function_flags = 0x0000_0400;
            data.code = vec![USToken::NativeParm as u8, 1];
        }
        let self_id = instance_of(&mut arena, class_id);
        let mut code = vec![USToken::VirtualFunction as u8, function_name as u8];
        for value in ["thread_0", "line_4096", "..\\Cutscenes\\Missing.txt"] {
            code.push(USToken::StringConst as u8);
            code.extend_from_slice(value.as_bytes());
            code.push(0);
        }
        code.push(USToken::EndFunctionParms as u8);
        code.push(USToken::Stop as u8);
        let registry = registry();
        let mut frame = Frame::new(&arena, &registry, &code, Some(self_id));
        frame.run().expect("Localize request");
        assert!(matches!(
            frame.take_suspend(),
            Some(Suspend::EffectResult { request_id: 0 })
        ));
        assert_eq!(
            frame.take_effects(),
            vec![ScriptEffect::LocalizeRequest {
                request_id: 0,
                section: "thread_0".to_string(),
                key: "line_4096".to_string(),
                package: "..\\Cutscenes\\Missing.txt".to_string(),
            }]
        );
    }

    #[test]
    fn cutscript_get_next_line_stops_before_full_array_endpoint() {
        use crate::arena::{ClassData, ObjectData, PropertyData, UObject};

        let mut arena = ObjectArena::new();
        let root = arena.root_for("HGame");
        let class_name = arena.names.intern("CutScriptDisk");
        let class = arena.alloc(UObject {
            name_index: class_name,
            outer: Some(root),
            data: ObjectData::Class(ClassData::default()),
            ..Default::default()
        });
        let function_name = arena.names.intern("GetNextLine");
        let function = arena.alloc(UObject {
            name_index: function_name,
            outer: Some(class),
            data: ObjectData::Function(Box::default()),
            ..Default::default()
        });
        let cursor_name = arena.names.intern("curScriptLine");
        arena.alloc(UObject {
            name_index: cursor_name,
            outer: Some(class),
            data: ObjectData::Property(Box::new(PropertyData {
                kind: PropertyKind::Int,
                links: Default::default(),
                array_dim: 1,
                property_flags: 0,
                category: 0,
            })),
            ..Default::default()
        });
        let array_name = arena.names.intern("lineArray");
        arena.alloc(UObject {
            name_index: array_name,
            outer: Some(class),
            data: ObjectData::Property(Box::new(PropertyData {
                kind: PropertyKind::Str,
                links: Default::default(),
                array_dim: 4096,
                property_flags: 0,
                category: 0,
            })),
            ..Default::default()
        });
        let actor_name = arena.names.intern("Disk");
        let mut properties = PropStore::new();
        properties.set(cursor_name, PropValue::Int(4096));
        properties.set(
            array_name,
            PropValue::FixedArray(vec![PropValue::Str("filled".to_string()); 4096]),
        );
        let actor = arena.alloc(UObject {
            name_index: actor_name,
            outer: Some(root),
            class_id: Some(class),
            data: ObjectData::Properties(properties),
            ..Default::default()
        });
        let output_name = arena.names.intern("Line");
        let registry = registry();
        let mut frame = Frame::new(&arena, &registry, &[USToken::Stop as u8], Some(actor));
        frame
            .locals
            .set(output_name, PropValue::Str("stale".to_string()));
        let mut args = CallArgs::default();
        args.push(CallArg {
            name: Some(output_name),
            value: PropValue::Str("stale".to_string()),
            lvalue: Some(LValue::Local(output_name)),
        });
        let callee = ResolvedCallee {
            function_id: function,
            params: Vec::new(),
            native_index: 0,
        };
        assert!(
            frame
                .cutscript_disk_endpoint(&callee, &args)
                .expect("endpoint guard")
        );
        assert_eq!(frame.result, PropValue::Bool(false));
        assert_eq!(
            frame.locals.get(output_name),
            Some(&PropValue::Str(String::new()))
        );
        assert_eq!(
            arena
                .get(actor)
                .unwrap()
                .properties()
                .unwrap()
                .get(array_name),
            Some(&PropValue::FixedArray(vec![
                PropValue::Str(
                    "filled".to_string()
                );
                4096
            ]))
        );
        drop(frame);
        arena
            .get_mut(actor)
            .unwrap()
            .properties_mut()
            .unwrap()
            .set(cursor_name, PropValue::Int(-1));
        let mut negative = Frame::new(&arena, &registry, &[USToken::Stop as u8], Some(actor));
        let fail = negative
            .cutscript_disk_endpoint(&callee, &args)
            .expect_err("negative cursor must remain loud");
        assert_eq!(fail.reason_code, "vm.array_out_of_bounds");
    }

    #[test]
    fn deferred_native_diagnostic_names_slot_and_registered_subject() {
        let arena = ObjectArena::new();
        let mut registry = NativeRegistry::new();
        registry
            .register(
                0x70,
                0,
                crate::natives::deferred_native,
                "Engine.Actor.PlaySound",
            )
            .expect("register deferred probe");
        let code = [0x70, USToken::EndFunctionParms as u8, USToken::Stop as u8];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        let fail = frame.run().expect_err("deferred native must remain loud");
        assert_eq!(fail.reason_code, "native.body_deferred");
        assert!(fail.message.contains("slot=0x0070"));
        assert!(fail.message.contains("Engine.Actor.PlaySound"));
    }

    #[test]
    fn omitted_arguments_resolve_from_class_defaults() {
        use crate::arena::{ObjectData, PropertyData, UObject};
        let (mut arena, class_id) = script_class_arena();
        let health_name = arena.names.intern("Health");
        let sum_name = arena.names.intern("Sum");
        let a_name = arena.names.intern("a");
        let default_name = arena.names.intern("Default__TestClass");

        // Class default object carrying Health = 99.
        let default_object = arena.alloc(UObject {
            name_index: default_name,
            class_id: Some(class_id),
            data: ObjectData::Properties(crate::props::PropStore::new()),
            ..Default::default()
        });
        if let ObjectData::Class(d) = &mut arena.get_mut(class_id).unwrap().data {
            d.default_object = Some(default_object);
        }
        if let ObjectData::Properties(store) = &mut arena.get_mut(default_object).unwrap().data {
            store.set(health_name, PropValue::Int(99));
        }
        // Function Sum(a): return a + b where `b` is declared as an int
        // parameter NAMED "Health" so its omitted-argument default resolves
        // from the class default object.
        let int_prop = |arena: &mut ObjectArena, name: u32, outer: ObjectId| {
            arena.alloc(UObject {
                name_index: name,
                outer: Some(outer),
                data: ObjectData::Property(Box::new(PropertyData {
                    kind: crate::value::PropertyKind::Int,
                    links: Default::default(),
                    array_dim: 1,
                    property_flags: 0,
                    category: 0,
                })),
                ..Default::default()
            })
        };
        let sum_id = arena.alloc(UObject {
            name_index: sum_name,
            outer: Some(class_id),
            data: ObjectData::Function(Box::default()),
            ..Default::default()
        });
        let pa = int_prop(&mut arena, a_name, sum_id);
        let pb = int_prop(&mut arena, health_name, sum_id);
        let resolver =
            resolver_with_objects(&mut arena, &[(a_name as i32, pa), (health_name as i32, pb)]);
        if let ObjectData::Function(f) = &mut arena.get_mut(sum_id).unwrap().data {
            f.params = vec![pa, pb];
            f.code = vec![
                // Add(a, b) — slot 147 = Add_IntInt per registry_with_core_ops
                147,
                USToken::LocalVariable as u8,
                a_name as u8,
                USToken::LocalVariable as u8,
                health_name as u8,
                USToken::EndFunctionParms as u8,
                USToken::Return as u8,
                USToken::Nothing as u8,
            ];
            f.resolver = resolver;
        }
        let self_id = instance_of(&mut arena, class_id);
        let registry = registry_with_core_ops();
        // Call Sum(10) with the second argument OMITTED: it binds by name
        // from the class default object (Health = 99).
        let code = [
            USToken::VirtualFunction as u8,
            sum_name as u8,
            USToken::IntConstByte as u8,
            10,
            USToken::EndFunctionParms as u8,
            USToken::Return as u8,
            USToken::Nothing as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, Some(self_id));
        assert_eq!(frame.run().expect("run"), PropValue::Int(109));
    }

    #[test]
    fn trace_iterator_preserves_actor_location_and_normal_across_next_and_pop() {
        let mut arena = ObjectArena::new();
        let first = arena.alloc(crate::arena::UObject::default());
        let second = arena.alloc(crate::arena::UObject::default());
        let registry = registry();
        let code = [
            3,
            0,
            USToken::IteratorNext as u8,
            USToken::IteratorPop as u8,
        ];
        let mut frame = Frame::new(&arena, &registry, &code, None);
        frame.runtime_offsets = (0..code.len()).map(|offset| (offset, offset)).collect();
        frame.pending_expression = Some(PendingExpression::BeginTraceIterator {
            actor: LValue::Local(10),
            hit_location: LValue::Local(11),
            hit_normal: LValue::Local(12),
        });
        let vector = |value: [f32; 3]| PropValue::Struct {
            struct_name: 20,
            fields: value
                .into_iter()
                .enumerate()
                .map(|(index, value)| (30 + index as u32, PropValue::Float(value)))
                .collect(),
        };
        frame.result = PropValue::Array(vec![
            PropValue::FixedArray(vec![
                PropValue::Object(Some(first.0 as i32)),
                vector([1.0, 2.0, 3.0]),
                vector([1.0, 0.0, 0.0]),
            ]),
            PropValue::FixedArray(vec![
                PropValue::Object(Some(second.0 as i32)),
                vector([4.0, 5.0, 6.0]),
                vector([0.0, 1.0, 0.0]),
            ]),
        ]);

        frame
            .resume_pending_expression()
            .expect("activate deferred TraceActors iterator");
        assert_eq!(
            frame.locals.get(10),
            Some(&PropValue::Object(Some(first.0 as i32)))
        );
        assert_eq!(frame.locals.get(11), Some(&vector([1.0, 2.0, 3.0])));
        assert_eq!(frame.locals.get(12), Some(&vector([1.0, 0.0, 0.0])));

        let cursors = frame.take_iterators();
        let locals = std::mem::take(&mut frame.locals);
        let mut resumed = Frame::new(&arena, &registry, &code, None);
        resumed.set_pc(2);
        resumed.locals = locals;
        resumed.restore_iterators(cursors);

        resumed
            .run()
            .expect("restored IteratorNext and iterator exhaustion");
        assert_eq!(resumed.locals.get(10), Some(&PropValue::Object(None)));
        assert_eq!(resumed.locals.get(11), Some(&vector([4.0, 5.0, 6.0])));
        assert_eq!(resumed.locals.get(12), Some(&vector([0.0, 1.0, 0.0])));
        assert!(resumed.iterators.is_empty());
    }
}
