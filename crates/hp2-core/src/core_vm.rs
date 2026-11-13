//! Direct Core event execution and canonical Core VM native binding.

use crate::canonical_vm_native::{CanonicalVmNative, VmNativeSpec};
use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::core_native_catalog;
use crate::object_owner::{CanonicalNativeBinding, OwnedFunctionRecord};
use crate::vm_owner::{VmExecutor, VmHost};
use hp2_abi::engine::generated::{FFrame, TArrayRaw, UField, UFunction, UObject, UTexture};
use std::{cell::{Cell, RefCell}, ffi::c_void, marker::PhantomData, ptr};

/// A synchronous, post-script-call hook. The records and parameter storage are
/// borrowed only for the call. The hook must not retain them. It may reenter
/// the VM; calls made while the hook runs are not observed by this hook.
pub type ScriptPostCallHook = for<'a> unsafe fn(
    CoreAccess<'a>, *mut UObject, *mut UFunction, *const u8, usize,
) -> RuntimeResult<()>;

/// A pre-call hook may consume a void script call after validating its frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptPreCallDecision { Continue, Consume }
pub type ScriptPreCallHook = for<'a> unsafe fn(
    CoreAccess<'a>, *mut UObject, *mut UFunction, *const u8, usize,
) -> RuntimeResult<ScriptPreCallDecision>;

/// Enter and exit surround the original script body. Exit also runs on error.
/// Both callbacks run under the active VM record-lifetime contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptScopePhase { Enter, Exit { succeeded: bool } }
pub type ScriptScopeHook = for<'a> unsafe fn(
    CoreAccess<'a>, *mut UObject, *mut UFunction, *const u8, usize, ScriptScopePhase,
) -> RuntimeResult<()>;
type ResolvedScopeSelector = for<'a> unsafe fn(CoreAccess<'a>, *mut UObject, *mut UFunction) -> RuntimeResult<bool>;
#[derive(Clone)]
enum ScriptScopeSelection { Fixed(Vec<(*mut UFunction, i32)>), Resolved(ResolvedScopeSelector) }
#[derive(Clone)]
struct ScriptScopeObserver { key: &'static str, selection: ScriptScopeSelection, callback: ScriptScopeHook }

/// A synchronous Rust replacement for an interpreted function body.
///
/// The canonical VM owns the frame, scope observers and post-call hooks. The
/// callback must not retain any record or frame, and all records must remain
/// stable through reentry, as for an interpreted body.
pub type ScriptBodyReplacement = for<'a> unsafe fn(
    CoreAccess<'a>, *mut UObject, *mut UFunction, *mut FFrame,
) -> RuntimeResult<ScriptPreCallDecision>;

/// A body replacement that receives the original VM return destination.
///
/// The destination is the VM's effective body result, including its return-local
/// slot when the caller discards the explicit result. The callback must validate
/// the reflected layout before writing, preserve direct-result aliasing and
/// partial writes, and never retain the destination.
pub type ScriptBodyResultReplacement = for<'a> unsafe fn(
    CoreAccess<'a>, *mut UObject, *mut UFunction, *mut FFrame, *mut c_void,
) -> RuntimeResult<ScriptPreCallDecision>;

/// Evaluates a plain input inside the original interpreted call's storage owner.
///
/// The callback consumes the current argument, writes its real destination and
/// preserves the original expression/error protocol. It must not retain any
/// frame, property, destination or borrowed execution capability. Its selected
/// unmanaged scalar ABI and operands must be admitted before body effects.
pub type ScriptPlainArgument = for<'owner, 'call, 'vm> unsafe fn(
    CoreAccess<'owner>, &'call ScriptCallArguments<'vm>, *mut FFrame,
    *mut hp2_abi::engine::generated::UProperty, *mut c_void,
) -> RuntimeResult<()>;

/// Evaluates one assignment value after the canonical Let resolves its lvalue.
///
/// `context` is the owner's current RHS context, and the destination is its
/// real lvalue or already-installed null-assignment sink. The callback consumes
/// the actual operand once, preserves its width/write/error protocol, and must
/// not retain the frame, destination, context or borrowed execution capability.
pub type ScriptAssignmentValue = for<'owner, 'call, 'vm> unsafe fn(
    CoreAccess<'owner>, &'call ScriptCallArguments<'vm>, *mut FFrame,
    *mut c_void, *mut c_void,
) -> RuntimeResult<()>;

/// Borrowed expression operations on the original scoped VM executor.
///
/// This capability cannot outlive its call-argument, assignment-value, or
/// native-tile float scope.
/// It holds no mutable borrow of host state during synchronous reentry.
pub struct ScriptCallArguments<'vm> {
    step: &'vm dyn Fn(*mut FFrame, *mut c_void) -> RuntimeResult<()>,
    dispatch: &'vm dyn Fn(u8, *mut c_void, *mut FFrame, *mut c_void) -> RuntimeResult<()>,
    finish: &'vm dyn Fn(*mut FFrame) -> RuntimeResult<()>,
    body_script_base: *const u8,
    body_script_length: usize,
}

impl std::fmt::Debug for ScriptCallArguments<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScriptCallArguments").finish_non_exhaustive()
    }
}

impl ScriptCallArguments<'_> {
    /// Returns the original body script identity captured by this scope's owner.
    ///
    /// This is not a query of the current frame or its live metadata. The tuple
    /// does not validate admission, own or keep storage alive, permit mutation,
    /// or authorize execution or use of a suspended frame. Establish admission,
    /// bounds and all existing storage/operand contracts before using offsets.
    /// Never retain this capability or its storage beyond the synchronous scope.
    #[must_use]
    pub fn body_script_snapshot(&self) -> (*const u8, usize) {
        (self.body_script_base, self.body_script_length)
    }

    /// Evaluates the next live argument through the same canonical executor.
    ///
    /// # Errors
    /// Returns the first canonical expression failure.
    ///
    /// # Safety
    /// The frame belongs to this synchronous invocation; its live cursor and
    /// result satisfy `VmExecutor::step_argument`. Do not step a suspended frame.
    pub unsafe fn step_argument(&self, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
        (self.step)(frame, result)
    }

    /// Dispatches an already consumed operand without replaying its opcode.
    ///
    /// # Errors
    /// Returns the canonical operand/native error, including expression returns.
    ///
    /// # Safety
    /// The opcode, context, frame and result are the current live operand of
    /// this invocation and satisfy the canonical VM dispatch contract.
    pub unsafe fn dispatch_operand(&self, opcode: u8, object: *mut c_void, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<()> {
        (self.dispatch)(opcode, object, frame, result)
    }

    /// Consumes the current argument terminator and optional debug expression.
    ///
    /// # Errors
    /// Returns the first canonical debug-expression failure.
    ///
    /// # Safety
    /// The current frame/cursor satisfies `VmExecutor::finish_arguments` in this
    /// invocation, including readable terminator and following opcode storage.
    pub unsafe fn finish_arguments(&self, frame: *mut FFrame) -> RuntimeResult<()> {
        (self.finish)(frame)
    }
}

/// A lexical bridge to the current native tile invocation's argument owners.
///
/// Private closures borrow the matching current frame. Each top-level input
/// creates its own canonical host; nested float work shares that input's host.
/// The scope owns no Canvas, texture, script, frame or destination storage.
pub struct NativeTileScope<'vm> {
    step_texture: &'vm dyn Fn(*mut *mut UTexture) -> RuntimeResult<()>,
    step_float: &'vm dyn Fn(*mut f32) -> RuntimeResult<()>,
    finish: &'vm dyn Fn() -> RuntimeResult<()>,
}

impl std::fmt::Debug for NativeTileScope<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("NativeTileScope").finish_non_exhaustive()
    }
}

impl NativeTileScope<'_> {
    /// Steps the actual native runner's pointer-typed texture input once.
    ///
    /// # Errors
    /// Returns the first canonical operand error, preserving earlier effects.
    ///
    /// # Safety
    /// Use only in this scope's currently executing native invocation and its
    /// original slot order. The actual pointer-width destination is initialized,
    /// writable and live through reentry. Do not retain it or this scope, or use
    /// a suspended frame's scope. The current expression fully writes its type.
    pub unsafe fn step_texture(&self, destination: *mut *mut UTexture) -> RuntimeResult<()> {
        (self.step_texture)(destination)
    }

    /// Steps the actual native runner's current float input once.
    ///
    /// # Errors
    /// Returns the first selected or canonical operand failure.
    ///
    /// # Safety
    /// Use only in this scope's currently executing native invocation and its
    /// original slot order. The actual f32 destination is initialized, writable
    /// and live through reentry. Current operands satisfy the canonical typed
    /// contract and this handoff's fully-writing requirement. No capability or destination can escape or
    /// operate a suspended parent; do not replace the actual native local.
    pub unsafe fn step_float(&self, destination: *mut f32) -> RuntimeResult<()> {
        (self.step_float)(destination)
    }

    /// Finishes the current native invocation with its independent fresh host.
    ///
    /// # Errors
    /// Returns the first terminator or optional DebugInfo failure.
    ///
    /// # Safety
    /// Call once after the real runner's seven successful input steps, while
    /// this invocation is current. The live terminator and following opcode
    /// satisfy the canonical finish contract. Do not retain or reuse the scope.
    pub unsafe fn finish_arguments(&self) -> RuntimeResult<()> {
        (self.finish)()
    }
}

/// Selects one actual float expression on its own fresh native-argument host.
///
/// The evaluator consumes its current fully-writing f32 expression once. It
/// uses the borrowed arguments for same-host children/finish and never retains
/// frames, capabilities, destination or staging addresses. Snapshot identity
/// does not grant admission, storage ownership, liveness or execution authority.
pub type NativeTileFloat = for<'owner, 'call, 'operand> unsafe fn(
    CoreAccess<'owner>, &'call ScriptCallArguments<'operand>, *mut FFrame, *mut f32,
) -> RuntimeResult<()>;

/// Enters the Engine-owned seven-slot runner for one selected invocation.
///
/// The Engine entry preserves actual locals, order, finish and resource leaves.
/// It uses only its supplied current scope; it cannot retain it or execute a
/// suspended parent. The receiver/frame/result obey the original native ABI.
pub type NativeTileEntry = for<'owner, 'call, 'native> unsafe fn(
    CoreAccess<'owner>, *mut UObject, *mut FFrame, *mut c_void,
    &'call NativeTileScope<'native>,
) -> RuntimeResult<()>;

impl CoreAccess<'_> {
    /// Steps an admitted Context/native tile call with a lexical input selector.
    ///
    /// The real Context owns receiver, null warning, header and skip effects.
    /// Only its inner phase at the admitted live native site selects the Engine
    /// entry. All other live continuations dispatch once without replay. The
    /// Engine retains its seven actual locals and real Canvas/resource leaves.
    ///
    /// # Errors
    /// Rejects an absent or mismatched current replacement scope before frame
    /// access. Other failures preserve canonical effects and ordered contexts.
    ///
    /// # Safety
    /// Call only from the presently executing replacement, after its ordinary
    /// statement guard, with that invocation's live frame/result. Never use a
    /// suspended parent. Before effects, admit the immutable original script
    /// header and bytes, actual Canvas/native ABI and selected input widths.
    /// The caller must also establish that every actual continuation under
    /// permitted reentry satisfies this API's fully-writing typed contract;
    /// this does not predict or freeze future cursors or data. Expected
    /// pointers identify sites only; selected reads derive from the captured
    /// original allocation, which remains live for its full length.
    /// Identity/bounds alone do not establish storage life.
    /// Records, input destinations and code remain valid through synchronous
    /// reentry. The caller must uniquely identify the actual reviewed installed
    /// aggregate descriptor before body effects. `entry` preserves the original
    /// Engine runner; `evaluate_float` uses only each borrowed fresh host. No
    /// scope, frame, scratch/staging address or capability can escape. Neither
    /// callback may publish a frame to an observer or use a suspended parent.
    /// A nested callback cannot reuse this invocation while it is suspended.
    /// The complete actual continuation, including later inputs and finish,
    /// must satisfy the current native ABI on every permitted reentry.
    /// Live data and metadata need their existing validity, not blanket immutability.
    pub unsafe fn execute_script_context_tile_with(
        self,
        frame: *mut FFrame,
        result: *mut c_void,
        expected_context: *const u8,
        expected_native: *const u8,
        expected_canvas: *mut UObject,
        expected_binding: &'static VmNativeSpec,
        entry: NativeTileEntry,
        evaluate_float: NativeTileFloat,
    ) -> RuntimeResult<crate::vm_owner::StepControl> {
        use crate::core_native_catalog::CoreNative;
        use crate::vm_owner::VmPropertyOp;
        const CONTEXT: u8 = 0x19;
        const TILE_HIGH: u8 = 0x61;
        const TILE_LOW: u8 = 0xd2;
        const TILE_INDEX: u16 = 466;
        #[derive(Clone, Copy)]
        enum ContextPhase { Receiver, Inner, Done }

        let active = self.script_body_scope(frame)?;
        let original_base = active.script_base.cast_const();
        let original_length = active.script_length;
        let snapshot_offset = |site: *const u8, width: usize| {
            if original_base.is_null() { return None; }
            let offset = (site as usize).checked_sub(original_base as usize)?;
            let end = offset.checked_add(width)?;
            (end <= original_length).then_some(offset)
        };
        let native_offset = snapshot_offset(expected_native, 2);
        let sites_valid = snapshot_offset(expected_context, 1).is_some() && native_offset.is_some();
        // SAFETY: the matching current-body contract retains this original host
        // through the synchronous operation; a pointer match alone is not authority.
        let host = unsafe { &*active.host.cast::<VmHostState<'_>>() };
        let _masked = ActiveScriptScope::mask(host);
        let vm = VmExecutor::new(host);
        // SAFETY: the caller supplies a live guarded frame and valid actual
        // continuations. Sample the live object before the outer opcode as step does.
        let outcome = unsafe {
            let object = (*frame).object;
            let outer_site = (*frame).code.cast_const();
            crate::vm_frame::step_with(frame, object, result, |opcode, object, frame, result| {
                if !sites_valid || outer_site != expected_context || opcode != CONTEXT {
                    return vm.dispatch(opcode, object, frame, result);
                }
                let mut phase = ContextPhase::Receiver;
                let outcome = execute_property_with(self, &host.lvalues, &host.assignment_sink, &vm,
                    VmPropertyOp::Context, object, frame, result,
                    &mut |next_frame, context, destination| {
                        let next_frame = next_frame.cast::<FFrame>();
                        let inner_phase = match phase {
                            ContextPhase::Receiver => { phase = ContextPhase::Inner; false },
                            ContextPhase::Inner => { phase = ContextPhase::Done; true },
                            ContextPhase::Done => false,
                        };
                        let native_site = (*next_frame).code.cast_const();
                        crate::vm_frame::step_with(next_frame, context, destination,
                            |inner, context, next_frame, destination| {
                                if !inner_phase || next_frame != frame
                                    || expected_canvas.is_null() || context != expected_canvas.cast()
                                    || native_site != expected_native || inner != TILE_HIGH
                                    || !self.effective_external_vm_native_is(TILE_INDEX, expected_binding)
                                {
                                    return vm.dispatch_operand(inner, context, next_frame, destination);
                                }
                                // The already-consumed high byte belongs to the
                                // admitted two-byte span. A mismatch leaves low
                                // unconsumed for canonical dispatch exactly once.
                                let Some(offset) = native_offset else {
                                    return vm.dispatch_operand(inner, context, next_frame, destination);
                                };
                                // SAFETY: derive from the retained original script
                                // allocation, not numeric identity pointers. Its
                                // admitted length includes this checked two-byte span.
                                let expected_low = original_base.add(offset + 1);
                                let low_site = (*next_frame).code.cast_const();
                                if low_site != expected_low || expected_low.read() != TILE_LOW {
                                    return vm.dispatch_operand(inner, context, next_frame, destination);
                                }
                                let _low = crate::vm_bytecode::read::<u8>(ptr::addr_of_mut!((*next_frame).code));
                                let step_texture = |value: *mut *mut UTexture| {
                                    // SAFETY: the Engine runner supplies its actual
                                    // initialized pointer local in current slot order.
                                    self.vm_step_argument(next_frame, value.cast())
                                };
                                let step_float = |value: *mut f32| {
                                    let input_host = VmHostState {
                                        access: self,
                                        lvalues: crate::vm_variable::EngineLvalueStorage::new(),
                                        assignment_sink: std::cell::UnsafeCell::new([0; 16]),
                                    };
                                    let input_vm = VmExecutor::new(&input_host);
                                    let step = |frame, result| input_vm.step_argument(frame, result);
                                    let dispatch = |opcode, object, frame, result| input_vm.dispatch_operand(opcode, object, frame, result);
                                    let finish = |frame| input_vm.finish_arguments(frame);
                                    let arguments = ScriptCallArguments {
                                        step: &step, dispatch: &dispatch, finish: &finish,
                                        body_script_base: original_base,
                                        body_script_length: original_length,
                                    };
                                    // SAFETY: the current input owns this fresh host
                                    // and real f32 destination through the callback.
                                    evaluate_float(self, &arguments, next_frame, value)
                                };
                                let finish = || self.vm_finish_arguments(next_frame);
                                let scope = NativeTileScope {
                                    step_texture: &step_texture, step_float: &step_float, finish: &finish,
                                };
                                // Preserve the ORIGINAL resolved handler identity,
                                // not the alternate entry's pointer, in the wrapper.
                                let original = CanonicalNativeBinding::External(expected_binding.handler);
                                entry(self, context.cast(), next_frame, destination, &scope)
                                    .map_err(|error| error.with_context(&format!("Core native {original:?} ({TILE_INDEX})")))
                            })
                    });
                outcome.map_err(|error| error.with_context(&format!("Core native {:?} (25)",
                    CanonicalNativeBinding::Core(CoreNative::intUObjectexecContext))))
            })
        };
        outcome.map_err(|error| error.with_context("stepping Core VM bytecode"))
    }
}

/// Original top-level statement state for a Rust-replaced interpreted body.
///
/// The 1024-byte statement result area has the same alignment and initial
/// FString-header clearing as `VmExecutor::run_script`. Keep one instance for
/// the whole body, including loops; never replace it with a null result slot.
pub struct ScriptBodyStatements {
    result: std::mem::MaybeUninit<[u128; 64]>,
    steps: u64,
}

impl ScriptBodyStatements {
    /// Starts the original zero-based guard count and statement result area.
    pub fn new() -> Self {
        let mut result = std::mem::MaybeUninit::<[u128; 64]>::uninit();
        // SAFETY: only the FString header is initialized, as in run_script.
        unsafe { result.as_mut_ptr().cast::<u8>().write_bytes(0, std::mem::size_of::<TArrayRaw>()); }
        Self { result, steps: 0 }
    }

    /// Captures, validates and guards one original body statement on its active host.
    ///
    /// `None` means the body finished, including the captured Return operand and
    /// optional debug expression. `Some(code)` is the pre-guard cursor for an
    /// already-guarded non-Return statement. Its live cursor can now differ.
    /// Execute that live statement once in Rust or with `execute_script_body_step`;
    /// do not guard it again or normalize a redirected live Return.
    ///
    /// # Errors
    /// Preserves the original range, guard, return-expression and debug failures.
    ///
    /// # Safety
    /// Use only in the currently executing replacement with its original frame
    /// and this body's persistent statement state. All script storage, records,
    /// typed operands and result destinations remain live through reentry.
    /// Never use a suspended outer scope. No caller statement effect precedes
    /// this operation; the guard and captured Return own their original effects.
    pub unsafe fn begin(&mut self, access: CoreAccess<'_>, frame: *mut FFrame) -> RuntimeResult<Option<*mut u8>> {
        let active = access.script_body_scope(frame)?;
        // SAFETY: the same current-body protocol as the explicit argument API
        // guarantees this erased host is borrowed by its live lexical guard.
        let host = unsafe { &*active.host.cast::<VmHostState<'_>>() };
        let _masked = ActiveScriptScope::mask(host);
        let vm = VmExecutor::new(host);
        if active.script_length == 0 { return Ok(None); }
        if active.script_base.is_null() { return Err(RuntimeError::new("Core VM function has script bytes but no script storage")); }
        // SAFETY: scope identity and the unsafe caller guarantee a live frame.
        let code = unsafe { (*frame).code };
        if code.is_null() { return Ok(None); }
        if (code as usize).wrapping_sub(active.script_base as usize) >= active.script_length {
            return Err(RuntimeError::new("Core VM instruction pointer is outside function script")
                .with_context(&format!("function {:#x}", active.function as usize)));
        }
        host.check_runaway(active.object, active.function, frame, self.steps)
            .map_err(|error| error.with_context("checking Core VM runaway guard"))?;
        self.steps = self.steps.wrapping_add(1);
        // SAFETY: the captured byte remains readable after the guard. Stock
        // tests this byte, not the potentially redirected live frame cursor.
        if unsafe { code.read() } == 4 {
            unsafe {
                (*frame).code = code.add(1);
                vm.step(frame, active.body_result)?;
            }
            // SAFETY: valid return/debug continuations preserve the snapshot's
            // original readable storage, even if metadata now names another range.
            let next = unsafe { (*frame).code };
            if !next.is_null()
                && (next as usize).wrapping_sub(active.script_base as usize) < active.script_length
                && unsafe { next.read() } == 0x38 {
                unsafe { vm.step(frame, active.body_result)?; }
            }
            return Ok(None);
        }
        Ok(Some(code))
    }

    /// Checks and counts one original top-level statement before its effects.
    ///
    /// # Errors
    /// Propagates the original statement guard failure without counting it.
    ///
    /// # Safety
    /// The frame must meet `CoreAccess::check_script_frame_statement`'s contract.
    /// Its cursor must identify this statement, not a nested expression.
    pub unsafe fn before(&mut self, access: CoreAccess<'_>, frame: *mut FFrame) -> RuntimeResult<()> {
        // SAFETY: the caller owns the original frame and statement position.
        unsafe { access.check_script_frame_statement(frame, self.steps)?; }
        self.steps = self.steps.wrapping_add(1);
        Ok(())
    }

    /// Borrows the original statement scratch area for synchronous VM stepping.
    ///
    /// The pointer must not outlive or move with this state and must not be
    /// retained by callbacks. Expression-specific writes remain unsafe.
    pub fn result(&mut self) -> *mut c_void { self.result.as_mut_ptr().cast() }
}

impl Default for ScriptBodyStatements {
    fn default() -> Self { Self::new() }
}

#[derive(Clone, Copy)]
enum ScriptBodyCallback {
    Body(ScriptBodyReplacement),
    Result(ScriptBodyResultReplacement),
}

#[derive(Clone)]
struct ScriptBodyBinding {
    key: &'static str,
    functions: Vec<(*mut UFunction, i32)>,
    callback: ScriptBodyCallback,
}

#[derive(Clone)]
struct ScriptPreObserver { key: &'static str, functions: Vec<(*mut UFunction, i32)>, callback: ScriptPreCallHook }

#[derive(Clone, Copy)]
struct ScriptHook {
    functions: [*mut UFunction; 3],
    callback: ScriptPostCallHook,
}
#[derive(Clone)]
struct ScriptObserver { key: &'static str, functions: Vec<(*mut UFunction, i32)>, callback: ScriptPostCallHook }
#[derive(Clone, Copy)]
struct ScriptRedirect { object: *mut UObject, object_index: i32, from: *mut UFunction, from_index: i32, to: *mut UFunction, to_index: i32 }
thread_local! {
    static SCRIPT_REDIRECT: Cell<Option<ScriptRedirect>> = const { Cell::new(None) };
    static SCRIPT_HOOK: Cell<Option<ScriptHook>> = const { Cell::new(None) };
    static SCRIPT_HOOK_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static SCRIPT_OBSERVERS: RefCell<Vec<ScriptObserver>> = const { RefCell::new(Vec::new()) };
    static SCRIPT_OBSERVERS_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static SCRIPT_BODY_BINDINGS: RefCell<Vec<ScriptBodyBinding>> = const { RefCell::new(Vec::new()) };
    static SCRIPT_PRE_OBSERVERS: RefCell<Vec<ScriptPreObserver>> = const { RefCell::new(Vec::new()) };
    static SCRIPT_PRE_OBSERVERS_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static SCRIPT_SCOPE_OBSERVERS: RefCell<Vec<ScriptScopeObserver>> = const { RefCell::new(Vec::new()) };
    static SCRIPT_SCOPE_CALLBACK_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

struct ScriptBodyGuard<'a> { key: &'static str, _access: PhantomData<CoreAccess<'a>>, _thread: PhantomData<std::rc::Rc<()>> }
impl Drop for ScriptBodyGuard<'_> {
    fn drop(&mut self) {
        SCRIPT_BODY_BINDINGS.with(|slot| slot.borrow_mut().retain(|binding| binding.key != self.key));
    }
}

struct ScriptScopeObserverGuard<'a> { key: &'static str, _access: PhantomData<CoreAccess<'a>>, _thread: PhantomData<std::rc::Rc<()>> }
impl Drop for ScriptScopeObserverGuard<'_> { fn drop(&mut self) { SCRIPT_SCOPE_OBSERVERS.with(|slot| slot.borrow_mut().retain(|item| item.key != self.key)); } }
struct ScriptScopeActiveGuard;
impl Drop for ScriptScopeActiveGuard { fn drop(&mut self) { SCRIPT_SCOPE_CALLBACK_ACTIVE.with(|active| active.set(false)); } }
struct ScriptPreObserverGuard<'a> { key: &'static str, _access: PhantomData<CoreAccess<'a>>, _thread: PhantomData<std::rc::Rc<()>> }
impl Drop for ScriptPreObserverGuard<'_> { fn drop(&mut self) { SCRIPT_PRE_OBSERVERS.with(|slot| slot.borrow_mut().retain(|item| item.key != self.key)); } }
struct ScriptPreObserversActiveGuard;
impl Drop for ScriptPreObserversActiveGuard { fn drop(&mut self) { SCRIPT_PRE_OBSERVERS_ACTIVE.with(|active| active.set(false)); } }
struct ScriptObserverGuard<'a> { key: &'static str, _access: PhantomData<CoreAccess<'a>>, _thread: PhantomData<std::rc::Rc<()>> }
impl Drop for ScriptObserverGuard<'_> { fn drop(&mut self) { SCRIPT_OBSERVERS.with(|slot| slot.borrow_mut().retain(|item| item.key != self.key)); } }
struct ScriptObserversActiveGuard;
impl Drop for ScriptObserversActiveGuard { fn drop(&mut self) { SCRIPT_OBSERVERS_ACTIVE.with(|active| active.set(false)); } }
struct ScriptRedirectGuard<'a>(PhantomData<CoreAccess<'a>>);
impl Drop for ScriptRedirectGuard<'_> { fn drop(&mut self) { SCRIPT_REDIRECT.with(|slot| slot.set(None)); } }
struct ScriptHookGuard<'a>(PhantomData<CoreAccess<'a>>);
impl Drop for ScriptHookGuard<'_> {
    fn drop(&mut self) { SCRIPT_HOOK.with(|hook| hook.set(None)); }
}
struct ScriptHookActiveGuard;
impl Drop for ScriptHookActiveGuard {
    fn drop(&mut self) { SCRIPT_HOOK_ACTIVE.with(|active| active.set(false)); }
}

impl<'a> CoreAccess<'a> {
    /// Replaces selected interpreted bodies without replacing VM call policy.
    ///
    /// Pre-observers still run first. Scope observers surround the replacement;
    /// normal successful-body observers and script post-call hooks still run.
    /// Nested calls use their normal observation policy, not the pre-hook guard.
    /// `Continue` runs the original body and is only valid before side effects;
    /// `Consume` completes a successfully replaced body.
    ///
    /// # Errors
    /// Rejects stale/native records, an empty selection, duplicate keys, or a
    /// function already bound by another replacement.
    ///
    /// # Safety
    /// The guard must drop before the Core owner and selected records. The
    /// callback must preserve the reflected frame layout and keep the receiver,
    /// function, script, frame and locals stable through synchronous reentry.
    /// This binding does not root objects or permit destructive GC reentry.
    pub unsafe fn install_script_body_replacement(
        self, key: &'static str, functions: &[*mut UFunction], callback: ScriptBodyReplacement,
    ) -> RuntimeResult<impl Drop + 'a + use<'a>> {
        // SAFETY: this preserves the caller's record and callback obligations.
        unsafe { self.install_body_callback(key, functions, ScriptBodyCallback::Body(callback)) }
    }

    /// Replaces a returning interpreted body with its original result address.
    ///
    /// This uses the same pre-observer, scope, post-call and error policy as
    /// `install_script_body_replacement`. No temporary return slot or delayed
    /// copy is introduced. `Continue` must occur before replacement effects.
    ///
    /// # Errors
    /// Rejects the same invalid selections as `install_script_body_replacement`,
    /// and functions without a reflected return destination.
    ///
    /// # Safety
    /// The caller must uphold `install_script_body_replacement`'s lifetime and
    /// reentry rules. The callback must validate the return layout and effective
    /// destination before writing. An ignored explicit result uses the VM's
    /// return-local slot, not a null pointer or temporary. A null destination
    /// must be rejected before effects. The callback must not retain or invalidate
    /// result storage. This binding adds no GC roots or destructive GC permission.
    pub unsafe fn install_script_result_body_replacement(
        self, key: &'static str, functions: &[*mut UFunction], callback: ScriptBodyResultReplacement,
    ) -> RuntimeResult<impl Drop + 'a + use<'a>> {
        // SAFETY: the result-aware callback inherits the documented obligations.
        unsafe { self.install_body_callback(key, functions, ScriptBodyCallback::Result(callback)) }
    }

    unsafe fn install_body_callback(
        self, key: &'static str, functions: &[*mut UFunction], callback: ScriptBodyCallback,
    ) -> RuntimeResult<impl Drop + 'a + use<'a>> {
        if functions.is_empty() { return Err(RuntimeError::new("Script body replacement requires functions")); }
        let mut entries = Vec::new();
        for &function in functions {
            if function.is_null() { return Err(RuntimeError::new("Null script body function")); }
            // SAFETY: the caller supplies stable canonical records.
            let index = unsafe { (*function.cast::<UObject>()).index };
            if index < 0 || self.object_at(index) != function.cast()
                || unsafe { (*function).function_flags } & crate::vm_dispatch::FUNC_NATIVE != 0 {
                return Err(RuntimeError::new("Invalid script body function"));
            }
            if matches!(callback, ScriptBodyCallback::Result(_))
                // SAFETY: the canonical live function was validated above.
                && unsafe { (*function).return_value_offset } == u16::MAX {
                return Err(RuntimeError::new("Result body replacement requires a return value"));
            }
            if !entries.iter().any(|&(candidate, _)| candidate == function) {
                entries.push((function, index));
            }
        }
        SCRIPT_BODY_BINDINGS.with(|slot| {
            let mut bindings = slot.borrow_mut();
            if bindings.iter().any(|binding| binding.key == key
                || binding.functions.iter().any(|(candidate, _)| entries.iter().any(|(function, _)| function == candidate))) {
                return Err(RuntimeError::new("Script body replacement already installed"));
            }
            bindings.push(ScriptBodyBinding { key, functions: entries, callback });
            Ok(ScriptBodyGuard { key, _access: PhantomData, _thread: PhantomData })
        })
    }

    /// Temporarily substitute one interpreted function for one live receiver.
    /// The caller must validate semantic and reflected frame compatibility.
    /// The guard must not outlive the owner or any of the supplied records.
    pub unsafe fn install_script_redirect(self, object: *mut UObject, from: *mut UFunction, to: *mut UFunction) -> RuntimeResult<impl Drop + 'a> {
        if object.is_null() || from.is_null() || to.is_null() { return Err(RuntimeError::new("Null script redirect record")); }
        let indices = unsafe { [(*object).index, (*from.cast::<UObject>()).index, (*to.cast::<UObject>()).index] };
        if [object, from.cast(), to.cast()].into_iter().zip(indices).any(|(p, i)| i < 0 || self.object_at(i) != p.cast())
            || unsafe { ((*from).function_flags | (*to).function_flags) & crate::vm_dispatch::FUNC_NATIVE != 0 } {
            return Err(RuntimeError::new("Invalid script redirect record"));
        }
        SCRIPT_REDIRECT.with(|slot| {
            if slot.get().is_some() { return Err(RuntimeError::new("Script redirect already installed")); }
            slot.set(Some(ScriptRedirect { object, object_index: indices[0], from, from_index: indices[1], to, to_index: indices[2] }));
            Ok(ScriptRedirectGuard(PhantomData))
        })
    }

    /// Surround selected script bodies with paired enter/exit callbacks.
    /// Callbacks must validate receiver and frame and cannot retain parameters.
    /// The thread-bound guard must be dropped before the owner and records.
    pub unsafe fn install_script_scope_observer(self, key: &'static str, functions: &[*mut UFunction], callback: ScriptScopeHook) -> RuntimeResult<impl Drop + 'a + use<'a>> {
        if functions.is_empty() { return Err(RuntimeError::new("Script scope observer requires functions")); }
        let mut entries = Vec::new();
        for &function in functions {
            if function.is_null() { return Err(RuntimeError::new("Null script scope observer function")); }
            let object = function.cast::<UObject>();
            let index = unsafe { (*object).index };
            if index < 0 || self.object_at(index) != object.cast() || unsafe { (*function).function_flags } & crate::vm_dispatch::FUNC_NATIVE != 0 {
                return Err(RuntimeError::new("Invalid script scope observer function"));
            }
            if !entries.iter().any(|&(p, _)| p == function) { entries.push((function, index)); }
        }
        SCRIPT_SCOPE_OBSERVERS.with(|slot| {
            let mut observers = slot.borrow_mut();
            if observers.iter().any(|item| item.key == key) { return Err(RuntimeError::new("Script scope observer key already installed")); }
            observers.push(ScriptScopeObserver { key, selection: ScriptScopeSelection::Fixed(entries), callback });
            Ok(ScriptScopeObserverGuard { key, _access: PhantomData, _thread: PhantomData })
        })
    }

    /// Select resolved interpreted calls and surround their bodies with paired callbacks.
    /// Selection and callbacks run synchronously, without an observer-list borrow.
    /// Nested calls made inside a selector or callback are not observed by scope
    /// observers, as with the fixed-list API. When an entered resolved observer
    /// and script execution both fail, the script error stays primary and the
    /// exit error is retained as diagnostic context.
    ///
    /// # Safety
    /// The guard must be dropped before the Core owner. Selectors and callbacks
    /// must not retain the supplied records or parameter storage. They may reenter
    /// only if the active VM's receiver, function, script, frame, locals and other
    /// active borrows remain live and stable through all reentry and until VM
    /// execution and cleanup finish. In particular, a collection must not collect
    /// or tear down these records; callers must ensure suitable roots. This API
    /// does not root active VM records or make their destruction safe.
    pub unsafe fn install_resolved_script_scope_observer(
        self, key: &'static str,
        selector: for<'b> unsafe fn(CoreAccess<'b>, *mut UObject, *mut UFunction) -> RuntimeResult<bool>,
        callback: ScriptScopeHook,
    ) -> RuntimeResult<impl Drop + 'a + use<'a>> {
        SCRIPT_SCOPE_OBSERVERS.with(|slot| {
            let mut observers = slot.borrow_mut();
            if observers.iter().any(|item| item.key == key) { return Err(RuntimeError::new("Script scope observer key already installed")); }
            observers.push(ScriptScopeObserver { key, selection: ScriptScopeSelection::Resolved(selector), callback });
            Ok(ScriptScopeObserverGuard { key, _access: PhantomData, _thread: PhantomData })
        })
    }

    /// Observe selected interpreted calls before their script bodies execute.
    /// The callback may consume only a void call whose out parameters it has
    /// handled. It must validate receiver identity and reflected frame layout.
    /// Records must remain live until this thread-bound guard is dropped.
    pub unsafe fn install_script_pre_observer(self, key: &'static str, functions: &[*mut UFunction], callback: ScriptPreCallHook) -> RuntimeResult<impl Drop + 'a + use<'a>> {
        if functions.is_empty() { return Err(RuntimeError::new("Script pre-observer requires functions")); }
        let mut entries = Vec::new();
        for &function in functions {
            if function.is_null() { return Err(RuntimeError::new("Null script pre-observer function")); }
            let object = function.cast::<UObject>();
            let index = unsafe { (*object).index };
            if index < 0 || self.object_at(index) != object.cast() || unsafe { (*function).function_flags } & crate::vm_dispatch::FUNC_NATIVE != 0 {
                return Err(RuntimeError::new("Invalid script pre-observer function"));
            }
            if !entries.iter().any(|&(p, _)| p == function) { entries.push((function, index)); }
        }
        SCRIPT_PRE_OBSERVERS.with(|slot| {
            let mut observers = slot.borrow_mut();
            if observers.iter().any(|item| item.key == key) { return Err(RuntimeError::new("Script pre-observer key already installed")); }
            observers.push(ScriptPreObserver { key, functions: entries, callback });
            Ok(ScriptPreObserverGuard { key, _access: PhantomData, _thread: PhantomData })
        })
    }

    /// Install an independent scoped observer alongside the legacy post-call
    /// hook. Callbacks must validate receiver identity and reflected frames.
    /// The owner, functions and callback must outlive the returned guard.
    pub unsafe fn install_script_observer(self, key: &'static str, functions: &[*mut UFunction], callback: ScriptPostCallHook) -> RuntimeResult<impl Drop + 'a + use<'a>> {
        if functions.is_empty() { return Err(RuntimeError::new("Script observer requires functions")); }
        let mut entries = Vec::new();
        for &function in functions {
            if function.is_null() { return Err(RuntimeError::new("Null script observer function")); }
            let object = function.cast::<UObject>();
            let index = unsafe { (*object).index };
            if index < 0 || self.object_at(index) != object.cast() || unsafe { (*function).function_flags } & crate::vm_dispatch::FUNC_NATIVE != 0 {
                return Err(RuntimeError::new("Invalid script observer function"));
            }
            if !entries.iter().any(|&(p, _)| p == function) { entries.push((function, index)); }
        }
        SCRIPT_OBSERVERS.with(|slot| {
            let mut observers = slot.borrow_mut();
            if observers.iter().any(|item| item.key == key) { return Err(RuntimeError::new("Script observer key already installed")); }
            observers.push(ScriptObserver { key, functions: entries, callback });
            Ok(ScriptObserverGuard { key, _access: PhantomData, _thread: PhantomData })
        })
    }

    /// Install one engine-thread hook for exactly 2 reflected script functions.
    /// Drop the returned guard before shutting down the owner or unloading its
    /// packages. Installation fails rather than replacing an existing hook.
    ///
    /// # Safety
    /// Both functions must be live, non-native records belonging to this owner
    /// until the guard is dropped. The callback must validate receiver class and
    /// reflected parameter layout before use, and must not retain raw borrows.
    pub unsafe fn install_script_post_call_hook(
        self, functions: [*mut UFunction; 2], callback: ScriptPostCallHook,
    ) -> RuntimeResult<impl Drop + 'a> {
        unsafe { self.install_script_post_call_hook_three([functions[0], functions[1], functions[1]], callback) }
    }

    /// Install a hook for three live interpreted functions on the engine thread.
    /// The same lifetime and reentrancy rules as `install_script_post_call_hook` apply.
    pub unsafe fn install_script_post_call_hook_three(
        self, functions: [*mut UFunction; 3], callback: ScriptPostCallHook,
    ) -> RuntimeResult<impl Drop + 'a> {
        if functions.iter().any(|f| f.is_null()) {
            return Err(RuntimeError::new("Script hook requires 3 functions"));
        }
        for function in functions {
            let object = function.cast::<UObject>();
            if self.object_at(unsafe { (*object).index }) != object.cast()
                || unsafe { (*function).function_flags } & crate::vm_dispatch::FUNC_NATIVE != 0
            {
                return Err(RuntimeError::new("Script hook function is foreign or native"));
            }
        }
        SCRIPT_HOOK.with(|slot| {
            if slot.get().is_some() {
                return Err(RuntimeError::new("Script hook already installed on this thread"));
            }
            slot.set(Some(ScriptHook { functions, callback }));
            Ok(ScriptHookGuard(PhantomData))
        })
    }
}

fn script_post_call_hook(
    access: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction,
    params: *const u8, size: usize,
) -> RuntimeResult<()> {
    let hook = SCRIPT_HOOK.with(Cell::get);
    let Some(hook) = hook.filter(|hook| hook.functions.contains(&function)) else { return Ok(()); };
    SCRIPT_HOOK_ACTIVE.with(|active| {
        if active.replace(true) { return Ok(()); }
        let _guard = ScriptHookActiveGuard;
        unsafe { (hook.callback)(access, object, function, params, size) }
    })
}

fn script_observers(access: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<()> {
    let observers = SCRIPT_OBSERVERS.with(|slot| slot.borrow().iter().filter(|item| item.functions.iter().any(|(candidate, _)| *candidate == function)).cloned().collect::<Vec<_>>());
    if observers.is_empty() { return Ok(()); }
    SCRIPT_OBSERVERS_ACTIVE.with(|active| {
        if active.replace(true) { return Ok(()); }
        let _guard = ScriptObserversActiveGuard;
        for item in observers {
            if let Some((_, index)) = item.functions.iter().find(|(candidate, _)| *candidate == function) {
                if access.object_at(*index) != function.cast() { return Err(RuntimeError::new("Stale script observer function")); }
                unsafe { (item.callback)(access, object, function, params, size)?; }
            }
        }
        Ok(())
    })
}

fn script_pre_observers(access: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<ScriptPreCallDecision> {
    let observers = SCRIPT_PRE_OBSERVERS.with(|slot| slot.borrow().iter().filter(|item| item.functions.iter().any(|(candidate, _)| *candidate == function)).cloned().collect::<Vec<_>>());
    if observers.is_empty() { return Ok(ScriptPreCallDecision::Continue); }
    SCRIPT_PRE_OBSERVERS_ACTIVE.with(|active| {
        if active.replace(true) { return Ok(ScriptPreCallDecision::Continue); }
        let _guard = ScriptPreObserversActiveGuard;
        for item in observers {
            if let Some((_, index)) = item.functions.iter().find(|(candidate, _)| *candidate == function) {
                if access.object_at(*index) != function.cast() { return Err(RuntimeError::new("Stale script pre-observer function")); }
                if unsafe { (item.callback)(access, object, function, params, size)? } == ScriptPreCallDecision::Consume {
                    if unsafe { (*function).return_value_offset } != u16::MAX { return Err(RuntimeError::new("Cannot consume a script call with a return value")); }
                    return Ok(ScriptPreCallDecision::Consume);
                }
            }
        }
        Ok(ScriptPreCallDecision::Continue)
    })
}

fn script_scope_callback(access: CoreAccess<'_>, item: &ScriptScopeObserver, object: *mut UObject, function: *mut UFunction, params: *const u8, size: usize, phase: ScriptScopePhase) -> RuntimeResult<()> {
    SCRIPT_SCOPE_CALLBACK_ACTIVE.with(|active| {
        if active.replace(true) { return Ok(()); }
        let _guard = ScriptScopeActiveGuard;
        unsafe { (item.callback)(access, object, function, params, size, phase) }
    })
}
fn script_scope_enter(access: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<Vec<ScriptScopeObserver>> {
    if SCRIPT_SCOPE_CALLBACK_ACTIVE.with(Cell::get) { return Ok(Vec::new()); }
    let candidates = SCRIPT_SCOPE_OBSERVERS.with(|slot| slot.borrow().iter().filter(|item| match &item.selection {
        ScriptScopeSelection::Fixed(functions) => functions.iter().any(|(candidate, _)| *candidate == function),
        ScriptScopeSelection::Resolved(_) => true,
    }).cloned().collect::<Vec<_>>());
    let mut entered: Vec<ScriptScopeObserver> = Vec::new();
    for item in candidates {
        let selected = match &item.selection {
            ScriptScopeSelection::Fixed(functions) => {
                match functions.iter().find(|(candidate, _)| *candidate == function) {
                    None => Ok(false),
                    Some((_, index)) if access.object_at(*index) != function.cast() => Err(RuntimeError::new("Stale script scope function")),
                    Some(_) => Ok(true),
                }
            }
            ScriptScopeSelection::Resolved(selector) => {
                if object.is_null() || function.is_null() { Err(RuntimeError::new("Null resolved script scope record")) }
                else {
                    let object_index = unsafe { (*object).index };
                    let function_index = unsafe { (*function.cast::<UObject>()).index };
                    if object_index < 0 || function_index < 0 || access.object_at(object_index) != object || access.object_at(function_index) != function.cast() {
                        Err(RuntimeError::new("Stale resolved script scope record"))
                    } else {
                        SCRIPT_SCOPE_CALLBACK_ACTIVE.with(|active| {
                            active.set(true);
                            let _guard = ScriptScopeActiveGuard;
                            unsafe { selector(access, object, function) }
                        })
                    }
                }
            }
        };
        let result = match selected { Ok(true) => script_scope_callback(access, &item, object, function, params, size, ScriptScopePhase::Enter), Ok(false) => continue, Err(error) => Err(error) };
        if let Err(error) = result {
            let resolved = matches!(item.selection, ScriptScopeSelection::Resolved(_)) || entered.iter().any(|scope| matches!(scope.selection, ScriptScopeSelection::Resolved(_)));
            let cleanup = script_scope_exit(access, &entered, object, function, params, size, false);
            return Err(match cleanup { Err(exit) if resolved => error.with_context(&format!("scope exit also failed: {exit}")), _ => error });
        }
        entered.push(item);
    }
    Ok(entered)
}
fn script_scope_exit(access: CoreAccess<'_>, entered: &[ScriptScopeObserver], object: *mut UObject, function: *mut UFunction, params: *const u8, size: usize, succeeded: bool) -> RuntimeResult<()> {
    let mut failure: Option<RuntimeError> = None;
    let resolved = entered.iter().any(|scope| matches!(scope.selection, ScriptScopeSelection::Resolved(_)));
    for item in entered.iter().rev() {
        if let Err(error) = script_scope_callback(access, item, object, function, params, size, ScriptScopePhase::Exit { succeeded }) {
            failure = Some(match failure { None => error, Some(first) if resolved => first.with_context(&format!("additional scope exit failed: {error}")), Some(first) => first });
        }
    }
    if let Some(error) = failure { Err(error) } else { Ok(()) }
}

fn script_body_binding(access: CoreAccess<'_>, function: *mut UFunction) -> RuntimeResult<Option<ScriptBodyCallback>> {
    SCRIPT_BODY_BINDINGS.with(|slot| {
        for binding in slot.borrow().iter() {
            if let Some((_, index)) = binding.functions.iter().find(|(candidate, _)| *candidate == function) {
                if access.object_at(*index) != function.cast() {
                    return Err(RuntimeError::new("Stale script body replacement"));
                }
                return Ok(Some(binding.callback));
            }
        }
        Ok(None)
    })
}

/// Applies UObject::ProcessEvent's source-ordered entry policy. VM function
/// calls use their separate FFrame dispatch path and must not pass through here.
fn accept_vm_event(
    access: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction, params: *mut c_void,
) -> RuntimeResult<bool> {
    crate::vm_dispatch::accept_event_with(|query| {
        use crate::vm_dispatch::EventEntryQuery::*;
        match query {
            Scriptable => Ok(access.is_scriptable()),
            Probing => unsafe {
                access.is_probing(object, i32::from_ne_bytes((*function.cast::<UObject>()).name))
            },
            PendingKill => unsafe { access.is_pending_kill(object) },
            IndexedNative => Ok(unsafe { (*function).i_native != 0 }),
            NativeFunction => Ok(unsafe { (*function).function_flags } & crate::vm_dispatch::FUNC_NATIVE != 0),
            RemoteConsumed => unsafe {
                access.process_remote_function(object, function, params, ptr::null_mut())
            },
        }
    })
}

impl CoreAccess<'_> {
    /// Calls an interpreted function from a live Rust-replaced script body.
    ///
    /// Uses the script-call path, not ProcessEvent's distinct entry policy.
    /// The existing VM owner retains locals, reentry and post-call ordering.
    ///
    /// # Errors
    /// Returns invalid-record, VM execution or teardown errors.
    ///
    /// # Safety
    /// `parent` is the current live replacement frame. Receiver, function,
    /// parameters and result satisfy the reflected layout and remain stable
    /// through reentry. Parameters must cover `function.parms_size` bytes.
    pub unsafe fn execute_script_callback(
        self, parent: *mut FFrame, object: *mut UObject, function: *mut UFunction,
        parameters: *mut c_void, result: *mut c_void,
    ) -> RuntimeResult<()> {
        if parent.is_null() || object.is_null() || function.is_null() {
            return Err(RuntimeError::new("Invalid script callback record"));
        }
        // Indexed native calls consume bytecode arguments from their caller,
        // not this pre-marshalled buffer. Keep that separate path unsupported.
        if unsafe { (*function).function_flags } & crate::vm_dispatch::FUNC_NATIVE != 0 {
            return Err(RuntimeError::new("Native script callback requires bytecode arguments"));
        }
        if crate::debug_trace::enabled() {
            crate::debug_trace::vm_call(self, object.cast(), function.cast());
        }
        let host = VmHostState {
            access: self,
            lvalues: crate::vm_variable::EngineLvalueStorage::new(),
            assignment_sink: std::cell::UnsafeCell::new([0; 16]),
        };
        // SAFETY: the caller supplies the live parent and reflected storage.
        unsafe { VmExecutor::new(&host).execute_event(object.cast(), function, parent, parameters.cast(), result) }
    }

    /// Executes a reflected function through the canonical Rust VM owner.
    ///
    /// # Errors
    /// Returns the first VM, host-operation, or cleanup error with its nested diagnostic context.
    ///
    /// # Safety
    /// `object` and `function` are live canonical records. Parameters and result satisfy the
    /// reflected function layout, and all pointed-to storage remains stable for synchronous VM
    /// reentry.
    pub(crate) unsafe fn execute_vm_event(
        self,
        object: *mut UObject,
        function: *mut UFunction,
        params: *mut c_void,
        result: *mut c_void,
    ) -> RuntimeResult<()> {
        if object.is_null() || function.is_null() {
            return Err(RuntimeError::new("Invalid Core VM event record"));
        }
        if !accept_vm_event(self, object, function, params)? {
            return Ok(());
        }
        // Emit the same spell/trigger trace hook as the bytecode `CallFunction`
        // path, so top-level dispatched events (Tick, TurnOn, TurnOff, CastSpell,
        // HandleSpell*) are visible alongside nested calls. Cheap atomic gate.
        if crate::debug_trace::enabled() {
            crate::debug_trace::vm_call(self, object.cast(), function.cast());
        }
        let host = VmHostState {
            access: self,
            lvalues: crate::vm_variable::EngineLvalueStorage::new(),
            assignment_sink: std::cell::UnsafeCell::new([0; 16]),
        };
        let executor = VmExecutor::new(&host);
        let trace_context = if self.vm_trace_enabled() {
            let actor = trace_path(&executor, object)?;
            let class = unsafe { (*object).class.cast::<UObject>() };
            let class = trace_path(&executor, class)?;
            let function_path = trace_path(&executor, function.cast())?;
            crate::ansi_trace::with_process_state(|trace| {
                crate::ansi_trace::direct_outer_scope_begin(
                    trace,
                    object.cast(),
                    function.cast(),
                    actor,
                    class,
                    function_path,
                )
            })
        } else {
            std::ptr::null_mut()
        };
        struct TraceContext(*mut crate::ansi_trace::Context);
        impl Drop for TraceContext {
            fn drop(&mut self) {
                // SAFETY: created exactly once for this event and ended by this guard.
                unsafe { crate::ansi_trace::context_scope_end(self.0) };
            }
        }
        let trace_context = TraceContext(trace_context);
        let result = unsafe {
            executor.execute_event(object.cast(), function, std::ptr::null_mut(), params.cast(), result)
        };
        drop(trace_context);
        result
    }

    /// Executes one bytecode opcode in a live canonical state frame.
    ///
    /// # Errors
    /// Returns the first direct Core VM or host-operation failure.
    ///
    /// # Safety
    /// `frame` points to the live `FFrame` prefix of an `FStateFrame`; its
    /// object, code, locals, parent and all bytecode operands remain valid for
    /// synchronous execution. `result` satisfies the current opcode's result
    /// layout, if that opcode writes one.
    pub unsafe fn execute_state_frame_step(
        self,
        frame: *mut FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<()> {
        if frame.is_null() {
            return Err(RuntimeError::new("Null Core state frame"));
        }
        // SAFETY: a live state frame provides the complete FFrame prefix.
        unsafe { self.execute_script_frame_step(frame, result) }
    }

    /// Executes one bytecode expression in a live canonical script frame.
    ///
    /// The VM step uses only `FFrame`, including for ordinary function frames.
    /// It does not require storage for an `FStateFrame` extension.
    ///
    /// # Errors
    /// Returns the first direct Core VM or host-operation failure.
    ///
    /// # Safety
    /// `frame` is a live `FFrame` owned by this Core. Its node, object, locals,
    /// parent chain and code operands remain valid during synchronous reentry.
    /// Its code points to a complete expression in the node's live script.
    /// `result` satisfies that expression's result layout if it writes one.
    pub unsafe fn execute_script_frame_step(
        self,
        frame: *mut FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<()> {
        if frame.is_null() {
            return Err(RuntimeError::new("Null Core script frame"));
        }
        let host = VmHostState {
            access: self,
            lvalues: crate::vm_variable::EngineLvalueStorage::new(),
            assignment_sink: std::cell::UnsafeCell::new([0; 16]),
        };
        let executor = VmExecutor::new(&host);
        unsafe { executor.step(frame, result)?; }
        Ok(())
    }

    /// Compares candidate identities to the current owner's body scope record.
    ///
    /// This read-only predicate never dereferences the candidate pointers,
    /// allocates, calls the VM or exposes the host/result. A missing or masked
    /// scope returns false. Success is a momentary identity comparison, not
    /// dynamic authority, a lifetime guarantee or permission to use a suspended
    /// outer frame. Actual execution still requires the current-body protocol.
    #[must_use]
    pub fn current_script_body_matches(
        self, frame: *mut FFrame, object: *mut UObject, function: *mut UFunction,
        script_base: *const u8, script_length: usize,
    ) -> bool {
        self.active_script_vm().is_some_and(|active| {
            !frame.is_null() && !active.host.is_null() && active.frame == frame
                && active.object == object.cast() && active.function == function
                && active.script_base.cast_const() == script_base
                && active.script_length == script_length
        })
    }

    /// Checks one scalar local against this event owner's captured allocation.
    ///
    /// Returns false for an absent/masked/mismatched current scope before any
    /// frame or property dereference. It invokes no callback and retains no
    /// record. Only Core IntProperty and FloatProperty value kinds are admitted.
    /// This is an instantaneous check, not a lifetime guarantee or authority
    /// to execute a suspended frame.
    ///
    /// # Safety
    /// With a matching current scope, `property` and its class must be live
    /// complete canonical records, as required by the actual VM operand. The
    /// caller in that matching scope must use the currently executing replacement,
    /// never a suspended outer invocation. A rejected identity query grants no
    /// authority to access that frame. No owner mutation may race this check. Further script
    /// identity, operand validity, initialized-value and no-callback conditions
    /// remain the caller's obligations before using any selected evaluator.
    pub unsafe fn current_script_scalar_local_matches(
        self, frame: *mut FFrame, property: *mut hp2_abi::engine::generated::UProperty,
        expected: crate::property_value::Hp2ValueKind, expected_offset: usize,
    ) -> bool {
        use crate::property_value::Hp2ValueKind::{ValueFloat, ValueInt};
        let Ok(active) = self.script_body_scope(frame) else { return false; };
        if property.is_null() || !matches!(expected, ValueInt | ValueFloat) { return false; }
        // SAFETY: the active scope and caller retain the current frame and record.
        let (node, locals, outer, offset, dim, element_size) = unsafe {
            ((*frame).node, (*frame).locals, (*property.cast::<UObject>()).outer,
             (*property).offset, (*property).array_dim, (*property).element_size)
        };
        if node != active.function.cast() || locals != active.locals_base
            || active.locals_base.is_null() || outer != active.function.cast()
            || dim != 1 || element_size != 4
            || usize::try_from(offset).ok() != Some(expected_offset)
            || expected_offset.checked_add(4).is_none_or(|end| end > active.locals_size)
            || (active.locals_base as usize).checked_add(expected_offset).is_none_or(|p| p % 4 != 0)
        { return false; }
        // SAFETY: the caller retains the property's actual complete class tail.
        unsafe { crate::core_property::value_kind(self, property) }.ok() == Some(expected)
    }

    /// Checks a byte span against this event owner's original parameter prefix.
    ///
    /// Rejects an absent, masked or nonmatching scope before frame access. It
    /// reads no parameter bytes or reflected metadata and invokes no callback.
    /// It does not expose the captured allocation or prove a field's identity.
    /// No alignment is required for a later unaligned read.
    ///
    /// # Safety
    /// For a matching scope, `frame` must be the currently executing live
    /// replacement frame. Never use a suspended outer invocation. The original
    /// event allocation remains live and no owner mutation races this check.
    pub unsafe fn current_script_parameter_span_matches(
        self, frame: *mut FFrame, offset: usize, width: usize,
    ) -> bool {
        let Ok(active) = self.script_body_scope(frame) else { return false; };
        // SAFETY: the caller supplies the current live frame for this scope.
        let (node, locals) = unsafe { ((*frame).node, (*frame).locals) };
        node == active.function.cast()
            && !active.locals_base.is_null()
            && locals == active.locals_base
            && active.parameter_size <= active.locals_size
            && offset.checked_add(width).is_some_and(|end| end <= active.parameter_size)
    }

    fn script_body_scope(self, frame: *mut FFrame) -> RuntimeResult<ActiveScriptVm> {
        self.active_script_vm()
            .filter(|active| !frame.is_null() && active.frame == frame && !active.host.is_null())
            .ok_or_else(|| RuntimeError::new("No matching active Core replacement frame"))
    }

    /// Executes one already-guarded live statement on the original body host.
    ///
    /// This is the generic non-captured-Return path after `ScriptBodyStatements::begin`.
    /// It preserves the live cursor and `StepControl`, including a redirected
    /// Return that must not evaluate a captured-Return operand. No guard is added.
    ///
    /// # Errors
    /// Rejects a missing/mismatched scope before frame access and returns the
    /// original step failure with its ordered contexts.
    ///
    /// # Safety
    /// For a matching scope, use the currently executing replacement's live,
    /// already-guarded frame and persistent statement result. All operands meet
    /// `VmExecutor::step`'s contract. Never execute a suspended outer frame.
    pub unsafe fn execute_script_body_step(self, frame: *mut FFrame, result: *mut c_void) -> RuntimeResult<crate::vm_owner::StepControl> {
        let active = self.script_body_scope(frame)?;
        // SAFETY: the matching active scope retains its original immutable
        // host through this synchronous call under the current-body contract.
        let host = unsafe { &*active.host.cast::<VmHostState<'_>>() };
        let _masked = ActiveScriptScope::mask(host);
        let vm = VmExecutor::new(host);
        // SAFETY: supplied by this method's already-guarded operand contract.
        unsafe { vm.step(frame, result) }
    }

    /// Evaluates one selected Let RHS inside its canonical lvalue/sink owner.
    ///
    /// The statement is already guarded. Only a matching live Let address and
    /// matching RHS address use `evaluate`, after the original LHS and possible
    /// null-assignment warning/sink setup. Other actual continuations dispatch
    /// once on the original host. No cursor rewind or second guard occurs.
    ///
    /// # Errors
    /// Rejects a missing/mismatched scope before frame access and preserves
    /// the original Let/step error contexts, order and partial writes.
    ///
    /// # Safety
    /// Use only in the currently executing replacement with its live frame and
    /// persistent statement result. Never use a suspended outer scope. Admit
    /// both immutable script identities and the selected unmanaged RHS width,
    /// alignment and fully writing typed operands before effects. Records, code
    /// and result destinations remain live through synchronous reentry. The
    /// evaluator consumes its current RHS once using the borrowed capability;
    /// it cannot reacquire the suspended parent body or retain any capability.
    pub unsafe fn execute_script_let_with_rhs(
        self, frame: *mut FFrame, result: *mut c_void,
        expected_let_opcode_ptr: *const u8, expected_rhs_opcode_ptr: *const u8,
        evaluate: ScriptAssignmentValue,
    ) -> RuntimeResult<crate::vm_owner::StepControl> {
        use crate::core_native_catalog::CoreNative;
        use crate::vm_owner::VmPropertyOp;
        let active = self.script_body_scope(frame)?;
        // SAFETY: the current-body protocol retains this exact immutable host
        // through its lexical guard; matching identity alone is not authority.
        let host = unsafe { &*active.host.cast::<VmHostState<'_>>() };
        let _masked = ActiveScriptScope::mask(host);
        let vm = VmExecutor::new(host);
        let step = |frame, result| {
            // SAFETY: borrowed-capability callers supply valid live operands.
            unsafe { vm.step_argument(frame, result) }
        };
        let dispatch = |opcode, object, frame, result| vm.dispatch_operand(opcode, object, frame, result);
        let finish = |frame| {
            // SAFETY: the evaluator supplies a live argument terminator.
            unsafe { vm.finish_arguments(frame) }
        };
        let arguments = ScriptCallArguments {
            step: &step, dispatch: &dispatch, finish: &finish,
            body_script_base: active.script_base.cast_const(),
            body_script_length: active.script_length,
        };
        // SAFETY: preserve step's live-object-before-opcode order. The canonical
        // Let owns both operand destinations and all warning/sink side effects.
        let outcome = unsafe {
            let object = (*frame).object;
            let code = (*frame).code;
            crate::vm_frame::step_with(frame, object, result, |opcode, object, frame, result| {
                if code.cast_const() != expected_let_opcode_ptr || opcode != 0x0f {
                    return vm.dispatch(opcode, object, frame, result);
                }
                let mut lhs = true;
                let outcome = execute_property_with(self, &host.lvalues, &host.assignment_sink, &vm,
                    VmPropertyOp::Let, object, frame, result,
                    &mut |operand_frame, context, destination| {
                        let operand_frame = operand_frame.cast::<FFrame>();
                        if lhs {
                            lhs = false;
                        } else if operand_frame == frame
                            && (*operand_frame).code.cast_const() == expected_rhs_opcode_ptr {
                            return evaluate(self, &arguments, operand_frame, context, destination);
                        }
                        crate::vm_frame::step_with(operand_frame, context, destination,
                            |opcode, object, frame, result| vm.dispatch_operand(opcode, object, frame, result))
                    });
                outcome.map_err(|error| error.with_context(&format!("Core native {:?} (15)",
                    CanonicalNativeBinding::Core(CoreNative::intUObjectexecLet))))
            })
        };
        outcome.map_err(|error| error.with_context("stepping Core VM bytecode"))
    }

    /// Runs a context call with lazy plain inputs inside its original call owner.
    ///
    /// Only the selected live final-opcode address and target use `evaluate`.
    /// Other current continuations use canonical dispatch without replay. The
    /// original Context and call owners retain receiver/argument order, storage,
    /// trace, event, cleanup, first error and result destination. No guard is added.
    ///
    /// # Errors
    /// Rejects absent/mismatched active replacement scope before VM effects and
    /// returns the first canonical operation failure with original error contexts.
    ///
    /// # Safety
    /// Scope absence or a nonmatching frame is rejected without dereferencing
    /// the frame. For a matching scope, call only from the presently executing
    /// body replacement with its supplied live frame/cursor and valid statement
    /// result. Frame equality is not dynamic authority: never use a suspended
    /// outer invocation from nested work, another owner, or a later scope. Admit the selected interpreted void
    /// target's unmanaged, non-out input ABI and operand identities before any
    /// body effect. Records and code remain valid across synchronous callbacks.
    /// `evaluate` consumes each current argument and preserves its expression,
    /// write and failure protocol; it must use the borrowed argument capability,
    /// not recursively obtain this operation for the suspended parent frame.
    pub unsafe fn execute_script_context_call_with_arguments(
        self,
        frame: *mut FFrame,
        result: *mut c_void,
        expected_final_opcode_ptr: *const u8,
        expected_function: *mut UFunction,
        evaluate: ScriptPlainArgument,
    ) -> RuntimeResult<crate::vm_owner::StepControl> {
        use crate::core_native_catalog::CoreNative;
        use crate::vm_owner::{StepControl, VmCallOp, VmPropertyOp};
        let active = self.script_body_scope(frame)?;
        // SAFETY: the owner-local scope holds a shared reference to this exact
        // host through the selected synchronous callback. The unsafe caller
        // supplies that currently executing callback's frame, not a suspended one.
        let host = unsafe { &*active.host.cast::<VmHostState<'_>>() };
        // Nested argument work cannot acquire this suspended body capability.
        // A nested selected replacement publishes/restores its own scope.
        let _masked = ActiveScriptScope::mask(host);
        let vm = VmExecutor::new(host);
        let step = |frame, result| {
            // SAFETY: callers of the borrowed capability provide live operands.
            unsafe { vm.step_argument(frame, result) }
        };
        let dispatch = |opcode, object, frame, result| vm.dispatch_operand(opcode, object, frame, result);
        let finish = |frame| {
            // SAFETY: the evaluator supplies its current argument terminator.
            unsafe { vm.finish_arguments(frame) }
        };
        let arguments = ScriptCallArguments {
            step: &step, dispatch: &dispatch, finish: &finish,
            body_script_base: active.script_base.cast_const(),
            body_script_length: active.script_length,
        };
        // SAFETY: the live replacement frame/cursor/result satisfy the ordinary
        // step contract. Canonical variable/call owners keep all storage local.
        let outcome = unsafe { crate::vm_frame::step_with(frame, (*frame).object, result,
            |opcode, object, frame, result| {
                if opcode != 0x19 { return vm.dispatch(opcode, object, frame, result); }
                let outcome = execute_property_with(self, &host.lvalues, &host.assignment_sink, &vm,
                    VmPropertyOp::Context, object, frame, result,
                    &mut |next_frame, context, destination| {
                        let next_frame = next_frame.cast::<FFrame>();
                        let selected = next_frame == frame
                            && (*next_frame).code.cast_const() == expected_final_opcode_ptr;
                        crate::vm_frame::step_with(next_frame, context, destination,
                            |inner, context, next_frame, destination| {
                                if !selected || inner != 0x1c {
                                    return vm.dispatch_operand(inner, context, next_frame, destination);
                                }
                                let outcome = execute_call_with(self, &vm, VmCallOp::Final,
                                    context, next_frame, destination,
                                    &|vm, function, parent, property, value| {
                                        if function == expected_function {
                                            evaluate(self, &arguments, parent, property, value)
                                        } else {
                                            vm.step_argument(parent, value)
                                        }
                                    });
                                outcome.map(|_: StepControl| ()).map_err(|error| error.with_context(
                                    &format!("Core native {:?} (28)", CanonicalNativeBinding::Core(CoreNative::intUObjectexecFinalFunction))))
                            })
                    });
                outcome.map_err(|error| error.with_context(
                    &format!("Core native {:?} (25)", CanonicalNativeBinding::Core(CoreNative::intUObjectexecContext))))
            }) };
        outcome.map_err(|error| error.with_context("stepping Core VM bytecode"))
    }

    /// Applies the original top-level statement guard for a replaced body.
    ///
    /// Call this immediately before each selected original statement, including
    /// branches, jumps and return. `steps` is that body's zero-based statement
    /// count. Expression steps and jump-opcode guards are separate; this does
    /// not execute an opcode or advance the frame. Preserve the original cursor
    /// and call order so guard logging and errors retain their VM attribution.
    ///
    /// # Errors
    /// Rejects a null frame and propagates the canonical host guard failure.
    ///
    /// # Safety
    /// The frame, receiver and function belong to this Core and remain live.
    /// Its cursor points to the original statement in validated script storage.
    pub unsafe fn check_script_frame_statement(
        self, frame: *mut FFrame, steps: u64,
    ) -> RuntimeResult<()> {
        if frame.is_null() { return Err(RuntimeError::new("Null Core script frame")); }
        // SAFETY: the caller supplies a stable, canonical interpreted frame.
        let (object, function) = unsafe { ((*frame).object, (*frame).node.cast::<UFunction>()) };
        check_runaway(self, object, function, frame, steps)
            .map_err(|error| error.with_context("checking Core VM runaway guard"))
    }

    /// Dispatches a latent action through the canonical Core VM native catalog.
    ///
    /// # Errors
    /// Returns the first direct Core VM or host-operation failure, including an
    /// unavailable native-index diagnostic.
    ///
    /// # Safety
    /// `object`, `frame`, and `result` satisfy the selected native's contract
    /// and remain valid for synchronous execution. `native_index` is the live
    /// state's exact `LatentAction` index.
    pub unsafe fn execute_vm_native_index(
        self,
        object: *mut UObject,
        native_index: u32,
        frame: *mut FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<()> {
        if object.is_null() || frame.is_null() {
            return Err(RuntimeError::new("Invalid Core VM latent-action context"));
        }
        let host = VmHostState {
            access: self,
            lvalues: crate::vm_variable::EngineLvalueStorage::new(),
            assignment_sink: std::cell::UnsafeCell::new([0; 16]),
        };
        let executor = VmExecutor::new(&host);
        let native = if let Some(spec) = core_native_catalog::by_opcode(native_index as i32) {
            CanonicalNativeBinding::Core(spec.native)
        } else if let Some(handler) = host.external_native(native_index)? {
            CanonicalNativeBinding::External(handler)
        } else {
            return Err(RuntimeError::new("No direct Core VM operation for latent-action index")
                .with_context(&format!("index {native_index:#06x}")));
        };
        unsafe {
            executor.execute_native(
                native,
                native_index,
                object.cast(),
                frame,
                result,
            )?;
        }
        Ok(())
    }

    /// Resets the process-wide runaway counter before a source script tick.
    ///
    /// This is `hp2_rs_core_vm_guard_reset` over the canonical Core owner.
    pub fn reset_script_runaway(self) {
        self.vm_reset_runaway();
    }

    /// Evaluates one argument using the canonical Core VM opcode dispatch.
    ///
    /// # Errors
    /// Returns the first bytecode, reflected-storage, or host-operation failure.
    ///
    /// # Safety
    /// `frame` points to a live canonical frame; `result` satisfies the
    /// expression's reflected layout and remains valid through synchronous execution.
    pub unsafe fn vm_step_argument(
        self, frame: *mut FFrame, result: *mut c_void,
    ) -> RuntimeResult<()> {
        if frame.is_null() {
            return Err(RuntimeError::new("Null Core VM frame"));
        }
        let host = VmHostState {
            access: self,
            lvalues: crate::vm_variable::EngineLvalueStorage::new(),
            assignment_sink: std::cell::UnsafeCell::new([0; 16]),
        };
        let executor = VmExecutor::new(&host);
        unsafe { executor.step_argument(frame, result) }
    }

    /// Steps one reference argument and returns its live lvalue address, or `scratch` for a
    /// value expression. The returned address is valid for the native call's synchronous use.
    ///
    /// # Safety
    /// `frame` points to a live canonical frame and `scratch` satisfies the argument layout.
    pub(crate) unsafe fn vm_step_argument_out(
        self,
        frame: *mut FFrame,
        scratch: *mut c_void,
    ) -> RuntimeResult<*mut c_void> {
        let host = VmHostState {
            access: self,
            lvalues: crate::vm_variable::EngineLvalueStorage::new(),
            assignment_sink: std::cell::UnsafeCell::new([0; 16]),
        };
        let executor = VmExecutor::new(&host);
        unsafe {
            host.lvalues.property_slot().write(std::ptr::null_mut());
            host.lvalues.address_slot().write(std::ptr::null_mut());
            executor.step_argument(frame, scratch)?;
            let address = host.lvalues.address_slot().read();
            Ok(if address.is_null() { scratch } else { address.cast() })
        }
    }

    pub(crate) unsafe fn vm_finish_arguments(self, frame: *mut FFrame) -> RuntimeResult<()> {
        let host = VmHostState {
            access: self,
            lvalues: crate::vm_variable::EngineLvalueStorage::new(),
            assignment_sink: std::cell::UnsafeCell::new([0; 16]),
        };
        let executor = VmExecutor::new(&host);
        unsafe { executor.finish_arguments(frame) }
    }
    /// Evaluates one reflected `FFrame::Step` expression at a replication condition offset.
    ///
    /// # Safety
    /// `object`, `owner_class`, `locals`, and the expression at `rep_offset` remain live and
    /// satisfy the bytecode operand's reflected storage requirements.
    pub unsafe fn eval_rep_condition(
        self,
        object: *mut UObject,
        owner_class: *mut hp2_abi::engine::generated::UClass,
        rep_offset: i32,
        locals: *mut u8,
    ) -> RuntimeResult<u32> {
        if object.is_null() || owner_class.is_null() {
            return Err(RuntimeError::new("Invalid replication condition context"));
        }
        let script = unsafe { ptr::addr_of!((*owner_class.cast::<hp2_abi::engine::generated::UStruct>()).script).read_unaligned() };
        if rep_offset < 0 || rep_offset >= script.num || script.data.is_null() {
            return Err(RuntimeError::new("Replication condition offset is outside reflected script"));
        }
        let mut frame = FFrame {
            node: owner_class.cast(),
            object: object.cast(),
            code: unsafe { script.data.cast::<u8>().add(rep_offset as usize) },
            locals,
            parent: ptr::null_mut(),
        };
        let mut result = 0u32;
        unsafe { self.vm_step_argument(&mut frame, (&mut result as *mut u32).cast())?; }
        Ok(result)
    }

    /// Calls the receiver's Main event with an owning FString parameter.
    ///
    /// # Safety
    /// The receiver is canonical and `parms` is terminated UTF-32 data live for this call.
    pub(crate) unsafe fn commandlet_main(self, object: *mut UObject, parms: *const u32) -> RuntimeResult<i32> {
        let mut len = 0;
        while unsafe { parms.add(len).read() } != 0 { len += 1; }
        let text = unsafe { std::slice::from_raw_parts(parms, len) };
        let mut value = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
        let outcome = (|| {
            if !text.is_empty() { write_string(&mut value, text)?; }
            unsafe { commandlet_main_owned(self, object, &mut value) }
        })();
        let cleanup = release_string(self, &mut value);
        match (outcome, cleanup) {
            (Ok(value), Ok(())) => Ok(value),
            (Ok(_), Err(error)) | (Err(error), Ok(())) => Err(error),
            (Err(error), Err(cleanup)) => Err(error.with_context(&cleanup.to_string())),
        }
    }

    /// Binds one function by its serialized native index and canonical declaring-class/name pair.
    ///
    /// # Errors
    /// Returns an error for an invalid function record or a native index/name/class mismatch.
    ///
    /// # Safety
    /// `function` is the live canonical `UFunction` allocation whose Rust-owned function tail is
    /// initialized by the object constructor.
    pub(crate) unsafe fn bind_function(self, function: *mut UFunction) -> RuntimeResult<()> {
        if function.is_null() {
            return Err(RuntimeError::new("Invalid Core function record"));
        }
        let (native_index, flags) = unsafe { ((*function).i_native, (*function).function_flags) };
        if flags & crate::class_metadata::METADATA_FUNC_NATIVE == 0 {
            if native_index != 0 {
                return Err(RuntimeError::new("Script function has a native index"));
            }
            unsafe { OwnedFunctionRecord::bind(function.cast(), None) };
            return Ok(());
        }
        // Checked Core intrinsics bind by index, not compiler-generated spelling
        // (for example StrStr versus StringString).
        if native_index != 0 {
            if let Some(native) = core_native_catalog::by_opcode(i32::from(native_index)) {
                unsafe { OwnedFunctionRecord::bind(function.cast(), Some(CanonicalNativeBinding::Core(native.native))) };
                return Ok(());
            }
            let (name, owner) = unsafe {
                (i32::from_ne_bytes((*function.cast::<UObject>()).name), (*function.cast::<UObject>()).outer)
            };
            let name = to_string(&self.name_text(name)?);
            let binding = external_binding(self, native_index, &name, owner)?
                .ok_or_else(|| RuntimeError::new("Unrecognized Core native function")
                    .with_context(&native_index.to_string()))?;
            unsafe { OwnedFunctionRecord::bind(function.cast(), Some(binding)) };
            return Ok(());
        }
        let (function_name, owner) = unsafe {
            (i32::from_ne_bytes((*function.cast::<UObject>()).name), (*function.cast::<UObject>()).outer)
        };
        let function_name = to_string(&self.name_text(function_name)?);
        let mut binding = None;
        for spec in core_native_catalog::iter().filter(|spec| spec.opcode.is_none() || spec.native == core_native_catalog::CoreNative::intUObjectexecNormalize) {
            let Some(native_name) = spec.token_name.rsplit("exec").next() else { continue; };
            if native_name != function_name { continue; }
            let class_name = spec.token_name.strip_prefix("int")
                .and_then(|name| name.split("exec").next())
                .and_then(|name| name.strip_prefix('U'))
                .ok_or_else(|| RuntimeError::new("Malformed Core native identity"))?;
            let base_class = self.native_class(class_name)?;
            if owner.is_null() || !unsafe { crate::object_model::is_child_of(owner.cast(), base_class.cast()) } {
                continue;
            }
            binding = Some(CanonicalNativeBinding::Core(spec.native));
            break;
        }
        if binding.is_none() {
            binding = external_binding(self, native_index, &function_name, owner)?;
        }
        // SAFETY: constructor publishes this function tail once per stable allocation.
        unsafe { OwnedFunctionRecord::bind(function.cast(), binding) };
        Ok(())
    }
}

fn external_binding(
    access: CoreAccess<'_>, native_index: u16, name: &str, owner: *mut c_void,
) -> RuntimeResult<Option<CanonicalNativeBinding>> {
    let index = (native_index != 0).then_some(native_index);
    if owner.is_null() { return Ok(None); }
    let mut binding = None;
    let mut nearest = usize::MAX;
    for spec in access.vm_natives().unwrap_or(&[]) {
        if spec.native_index != index || !spec.function_name.eq_ignore_ascii_case(name) {
            continue;
        }
        let base = access.native_class(spec.class_name)?;
        let mut class = owner;
        let mut distance = 0;
        while !class.is_null() && distance < nearest {
            if class == base.cast() {
                // The UFunction owner is its declaring class. A derived native
                // override must win over a catalog entry for its ancestor.
                binding = Some(CanonicalNativeBinding::External(spec.handler));
                nearest = distance;
                break;
            }
            class = unsafe { (*class.cast::<UField>()).super_field };
            distance += 1;
        }
        if nearest == 0 { break; }
    }
    Ok(binding)
}

fn to_string(units: &[u32]) -> String {
    units.iter().take_while(|&&unit| unit != 0)
        .filter_map(|&unit| char::from_u32(unit))
        .collect()
}

#[repr(C)]
struct CommandletMainParams { in_parms: TArrayRaw, return_value: i32 }

/// Transfers the private owning argument into event parameters and back, including
/// mutations committed before failure. No canonical record borrow spans execution.
unsafe fn commandlet_main_owned(
    access: CoreAccess<'_>, object: *mut UObject, value: &mut TArrayRaw,
) -> RuntimeResult<i32> {
    let class = unsafe { (*object).class.cast() };
    let main = access.intern_str("Main", crate::name_owner::FindName::Find)?;
    let function_class = access.native_class("Function")?;
    let function = unsafe { access.find_field(class, main, function_class) }.cast::<UFunction>();
    if function.is_null() { return Err(RuntimeError::new("Commandlet has no Main event")); }
    if usize::from(unsafe { (*function).parms_size }) > std::mem::size_of::<CommandletMainParams>()
        || usize::from(unsafe { (*function).return_value_offset }) != std::mem::offset_of!(CommandletMainParams, return_value)
    {
        return Err(RuntimeError::new("Invalid Commandlet.Main parameter layout"));
    }
    let mut parameters = CommandletMainParams {
        in_parms: std::mem::replace(value, TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 }),
        return_value: 0,
    };
    let outcome = unsafe {
        access.execute_vm_event(object, function, (&raw mut parameters).cast(), std::ptr::null_mut())
    };
    *value = parameters.in_parms;
    outcome.map(|()| parameters.return_value)
}

pub(crate) fn log_frame(
    access: CoreAccess<'_>, frame: *mut hp2_abi::engine::generated::FFrame,
    event: &str, text: &[u32],
) -> RuntimeResult<()> {
    // FOutputDevice::Logf formats into 4096 TCHARs; overflow clears the payload.
    log_frame_message(access, frame, event, if text.len() < 4096 { text } else { &[] })
}

/// FOutputDevice::Log skips the intermediate printf buffer used by Logf.
pub(crate) fn log_frame_message(
    access: CoreAccess<'_>, frame: *mut hp2_abi::engine::generated::FFrame,
    event: &str, text: &[u32],
) -> RuntimeResult<()> {
    let input = access.intern_str(event, crate::name_owner::FindName::Find)?;
    if access.name_suppressed(input) { return Ok(()); }
    let (object, node, code) = unsafe { ((*frame).object.cast(), (*frame).node.cast::<UObject>(), (*frame).code) };
    let mut message = crate::core_lifetime::object_full_name(access, object)?;
    message.extend(" (".chars().map(u32::from));
    message.extend(crate::core_lifetime::object_full_name(access, node)?);
    message.push(u32::from(b':'));
    let script = unsafe { (*node.cast::<hp2_abi::engine::generated::UStruct>()).script.data };
    let offset = (code as usize).wrapping_sub(script as usize) as u32;
    let digits = (32 - offset.leading_zeros()).div_ceil(4).max(4);
    for digit in (0..digits).rev() {
        message.push(u32::from(b"0123456789ABCDEF"[((offset >> (digit * 4)) & 15) as usize]));
    }
    message.extend(") ".chars().map(u32::from));
    message.extend_from_slice(text);
    if message.len() >= 4096 { message.clear(); }
    if event == "Critical" || access.is_strict() {
        return Err(RuntimeError::from_utf32(message));
    }
    let category = access.intern_str("ScriptWarning", crate::name_owner::FindName::Find)?;
    if access.name_suppressed(category) { Ok(()) } else { access.emit_log(category, &message) }
}

pub(crate) unsafe fn run_native_iterator(
    access: CoreAccess<'_>, frame: *mut FFrame,
    get_next: impl FnMut() -> RuntimeResult<bool>,
) -> RuntimeResult<()> {
    if frame.is_null() {
        return Err(RuntimeError::new("Null Core native iterator frame"));
    }
    let host = VmHostState {
        access,
        lvalues: crate::vm_variable::EngineLvalueStorage::new(),
        assignment_sink: std::cell::UnsafeCell::new([0; 16]),
    };
    let executor = VmExecutor::new(&host);
    unsafe {
        crate::vm_iterator::run(
            frame,
            |offset| {
                let node = (*frame).node.cast::<hp2_abi::engine::generated::UStruct>();
                if node.is_null() {
                    return Err(RuntimeError::new("Core iterator frame has no script node"));
                }
                let script = ptr::addr_of!((*node).script).read_unaligned();
                if script.data.is_null() || offset < 0 || offset > script.num {
                    return Err(RuntimeError::new("Core iterator end is outside its script"));
                }
                Ok(script.data.cast::<u8>().add(offset as usize))
            },
            |result| executor.step_argument(frame, result),
            get_next,
        )
    }
}

/// Erased only inside the owner-local lexical replacement scope.
#[derive(Clone, Copy)]
pub(crate) struct ActiveScriptVm {
    host: *const c_void,
    frame: *mut FFrame,
    object: *mut c_void,
    function: *mut UFunction,
    body_result: *mut c_void,
    script_base: *mut u8,
    script_length: usize,
    locals_base: *mut u8,
    locals_size: usize,
    parameter_size: usize,
}

struct ActiveScriptScope<'scope, 'owner> {
    host: &'scope VmHostState<'owner>,
    previous: Option<ActiveScriptVm>,
}

impl<'scope, 'owner> ActiveScriptScope<'scope, 'owner> {
    fn enter(host: &'scope VmHostState<'owner>, active: ActiveScriptVm) -> Self {
        Self { host, previous: host.access.replace_active_script_vm(Some(active)) }
    }

    fn mask(host: &'scope VmHostState<'owner>) -> Self {
        Self { host, previous: host.access.replace_active_script_vm(None) }
    }
}

impl Drop for ActiveScriptScope<'_, '_> {
    fn drop(&mut self) {
        self.host.access.replace_active_script_vm(self.previous);
    }
}

struct VmHostState<'a> {
    access: CoreAccess<'a>,
    lvalues: crate::vm_variable::EngineLvalueStorage,
    assignment_sink: std::cell::UnsafeCell<[u8; 16]>,
}

impl VmHost for VmHostState<'_> {
    fn object_at(&self, index: i32) -> RuntimeResult<*mut c_void> {
        let object = self.access.object_at(index);
        if object.is_null() && index != -1 {
            return Err(RuntimeError::new("Invalid Core VM object index").with_context(&index.to_string()));
        }
        Ok(object.cast())
    }

    fn native_binding(&self, function: *mut UFunction) -> RuntimeResult<CanonicalNativeBinding> {
        unsafe { OwnedFunctionRecord::binding(function.cast())? }
            .ok_or_else(|| RuntimeError::new("Core native function has no Rust binding"))
    }

    fn external_native(&self, opcode: u32) -> RuntimeResult<Option<CanonicalVmNative>> {
        let Ok(opcode) = u16::try_from(opcode) else { return Ok(None); };
        Ok(self.access.vm_natives().unwrap_or(&[]).iter()
            .find(|spec| spec.native_index == Some(opcode))
            .map(|spec| spec.handler))
    }

    unsafe fn execute_external_native(
        &self, native: CanonicalVmNative, object: *mut c_void,
        frame: *mut FFrame, result: *mut c_void,
    ) -> RuntimeResult<()> {
        unsafe { native(self.access, object.cast(), frame, result) }
    }

    fn name_text(&self, index: i32) -> RuntimeResult<Vec<u32>> { self.access.name_text(index) }
    fn find_name(&self, text: &[u32]) -> RuntimeResult<i32> { self.access.find_name(text) }
    fn none_name(&self) -> RuntimeResult<i32> { Ok(0) }

    fn log(&self, event: i32, text: &[u32]) -> RuntimeResult<()> { self.access.emit_log(event, text) }

    fn config_get(&self, section: &[u32], key: &[u32]) -> RuntimeResult<Option<Vec<u32>>> {
        self.access.config_get(section, key)
    }

    fn config_set(&self, section: &[u32], key: &[u32], value: &[u32]) -> RuntimeResult<()> {
        self.access.config_set(section, key, value)
    }

    fn archive_object_reference(&self, object: *mut c_void) -> RuntimeResult<*mut c_void> { Ok(object) }

    fn property_address(&self, object: *mut c_void, property: *mut c_void) -> RuntimeResult<*mut u8> {
        unsafe { self.access.property_address(object.cast(), property.cast()) }
    }

    fn reference_argument(
        &self,
        vm: &VmExecutor<'_, Self>,
        frame: *mut hp2_abi::engine::generated::FFrame,
        scratch: *mut c_void,
        size: usize,
    ) -> RuntimeResult<*mut c_void> {
        reference_argument(self.access, &self.lvalues, vm, frame, scratch, size)
    }

    fn assign_string_constant(
        &self,
        vm: &VmExecutor<'_, Self>,
        unicode: bool,
        frame: *mut hp2_abi::engine::generated::FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<()> {
        assign_string_constant(self.access, vm, unicode, frame, result)
    }

    fn allocate_locals(&self, layout: std::alloc::Layout) -> RuntimeResult<std::ptr::NonNull<u8>> {
        let raw = unsafe { std::alloc::alloc(layout) };
        std::ptr::NonNull::new(raw).ok_or_else(|| RuntimeError::new("Unable to allocate Core VM locals"))
    }

    fn free_locals(&self, locals: std::ptr::NonNull<u8>, layout: std::alloc::Layout) {
        unsafe { std::alloc::dealloc(locals.as_ptr(), layout) }
    }

    fn construct_locals(&self, function: *mut UFunction, locals: *mut u8) -> RuntimeResult<()> {
        construct_function_values(self.access, function, locals)
    }

    fn destroy_locals(&self, function: *mut UFunction, locals: *mut u8) -> RuntimeResult<()> {
        destroy_function_values(self.access, function, locals, false)
    }

    fn string_argument(
        &self,
        vm: &VmExecutor<'_, Self>,
        frame: *mut hp2_abi::engine::generated::FFrame,
        result: *mut hp2_abi::engine::generated::TArrayRaw,
    ) -> RuntimeResult<()> {
        string_argument(self.access, vm, frame, result)
    }

    fn release_string(&self, value: *mut hp2_abi::engine::generated::TArrayRaw) -> RuntimeResult<()> {
        release_string(self.access, value)
    }

    fn execute_property(
        &self,
        vm: &VmExecutor<'_, Self>,
        operation: crate::vm_owner::VmPropertyOp,
        object: *mut c_void,
        frame: *mut hp2_abi::engine::generated::FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<crate::vm_owner::StepControl> {
        execute_property(self.access, &self.lvalues, &self.assignment_sink, vm, operation, object, frame, result)
    }

    fn execute_call(
        &self,
        vm: &VmExecutor<'_, Self>,
        operation: crate::vm_owner::VmCallOp,
        object: *mut c_void,
        frame: *mut hp2_abi::engine::generated::FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<crate::vm_owner::StepControl> {
        execute_call(self.access, vm, operation, object, frame, result)
    }

    fn execute_system(
        &self,
        vm: &VmExecutor<'_, Self>,
        operation: crate::vm_owner::VmSystemOp,
        native_slot: u32,
        object: *mut c_void,
        frame: *mut hp2_abi::engine::generated::FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<crate::vm_owner::StepControl> {
        execute_system(self.access, vm, operation, native_slot, object, frame, result)
    }

    fn execute_string(
        &self,
        vm: &VmExecutor<'_, Self>,
        operation: crate::vm_owner::VmStringOp,
        opcode: u32,
        object: *mut c_void,
        frame: *mut hp2_abi::engine::generated::FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<crate::vm_owner::StepControl> {
        execute_string(self.access, vm, operation, opcode, object, frame, result)
    }

    fn execute_flow(
        &self,
        vm: &VmExecutor<'_, Self>,
        operation: crate::vm_owner::VmFlowOp,
        object: *mut c_void,
        frame: *mut hp2_abi::engine::generated::FFrame,
        result: *mut c_void,
    ) -> RuntimeResult<crate::vm_owner::StepControl> {
        execute_flow(self.access, vm, operation, object, frame, result)
    }

    fn redirect_function(&self, object: *mut c_void, function: *mut UFunction) -> RuntimeResult<*mut UFunction> {
        let redirect = SCRIPT_REDIRECT.with(Cell::get);
        let Some(r) = redirect.filter(|r| r.object.cast::<c_void>() == object && r.from == function) else { return Ok(function); };
        if self.access.object_at(r.object_index) != r.object.cast() || self.access.object_at(r.from_index) != r.from.cast() || self.access.object_at(r.to_index) != r.to.cast() {
            return Err(RuntimeError::new("Stale script redirect"));
        }
        Ok(r.to)
    }

    fn event_boundary(
        &self,
        phase: crate::vm_owner::EventBoundary,
        object: *mut c_void,
        function: *mut UFunction,
        frame: *mut hp2_abi::engine::generated::FFrame,
    ) -> RuntimeResult<()> {
        event_boundary(self.access, phase, object, function, frame)
    }

    fn interpreted_call_with_locals_snapshot(
        &self, object: *mut c_void, function: *mut UFunction,
        frame: *mut FFrame, result: *mut c_void,
        body_result: *mut c_void, from_script_call: bool,
        script_base: *mut u8, script_length: usize, locals_base: *mut u8, locals_size: usize, parameter_size: usize,
        execute: impl FnOnce() -> RuntimeResult<()>,
    ) -> RuntimeResult<()> {
        let mut executed = false;
        self.access.with_interpreted_call(
            object.cast(), function, frame, result, from_script_call, || {
                let params = unsafe { (*frame).locals.cast::<u8>() };
                let size = usize::from(unsafe { (*function).parms_size });
                if script_pre_observers(self.access, object.cast(), function, params, size)? == ScriptPreCallDecision::Consume { return Ok(()); }
                let scopes = script_scope_enter(self.access, object.cast(), function, params, size)?;
                let result = match script_body_binding(self.access, function) {
                    // SAFETY: the canonical VM owns these stable records and
                    // locals until this synchronous interpreted call returns.
                    Ok(Some(replace)) => {
                        // SAFETY: the canonical VM retains its original frame,
                        // records and result destination through this call. No
                        // binding-list borrow crosses synchronous reentry.
                        let decision = {
                            // Retain the exact host only while the selected
                            // callback executes, not Continue or exit observers.
                            let active = ActiveScriptVm {
                                host: (self as *const Self).cast(), frame, object,
                                function, body_result, script_base, script_length, locals_base, locals_size, parameter_size,
                            };
                            let _scope = ActiveScriptScope::enter(self, active);
                            // SAFETY: the canonical VM owns the current frame,
                            // body result and records throughout this callback.
                            unsafe { match replace {
                                ScriptBodyCallback::Body(callback) => callback(self.access, object.cast(), function, frame),
                                ScriptBodyCallback::Result(callback) => callback(self.access, object.cast(), function, frame, body_result),
                            } }
                        };
                        match decision {
                            Ok(ScriptPreCallDecision::Continue) => execute(),
                            Ok(ScriptPreCallDecision::Consume) => Ok(()),
                            Err(error) => Err(error),
                        }
                    },
                    Ok(None) => execute(),
                    Err(error) => Err(error),
                };
                let exit = script_scope_exit(self.access, &scopes, object.cast(), function, params, size, result.is_ok());
                let resolved = scopes.iter().any(|scope| matches!(scope.selection, ScriptScopeSelection::Resolved(_)));
                match (result, exit) {
                    (Err(error), Err(exit)) if resolved => return Err(error.with_context(&format!("scope exit also failed: {exit}"))),
                    (Err(error), _) => return Err(error),
                    (_, Err(error)) => return Err(error),
                    (Ok(()), Ok(())) => {}
                }
                executed = true;
                Ok(())
            },
        )?;
        if executed {
            let params = unsafe { (*frame).locals.cast::<u8>() };
            let size = usize::from(unsafe { (*function).parms_size });
            if from_script_call { script_post_call_hook(self.access, object.cast(), function, params, size)?; }
            script_observers(self.access, object.cast(), function, params, size)?;
        }
        Ok(())
    }

    fn check_runaway(
        &self,
        object: *mut c_void,
        function: *mut UFunction,
        frame: *mut hp2_abi::engine::generated::FFrame,
        steps: u64,
    ) -> RuntimeResult<()> {
        check_runaway(self.access, object, function, frame, steps)
    }
}

fn reference_argument<H: VmHost>(
    _access: CoreAccess<'_>,
    lvalues: &crate::vm_variable::EngineLvalueStorage,
    vm: &VmExecutor<'_, H>,
    frame: *mut hp2_abi::engine::generated::FFrame,
    scratch: *mut c_void,
    _size: usize,
) -> RuntimeResult<*mut c_void> {
    unsafe {
        lvalues.property_slot().write(std::ptr::null_mut());
        lvalues.address_slot().write(std::ptr::null_mut());
        vm.step_argument(frame, scratch)?;
        let address = lvalues.address_slot().read();
        Ok(if address.is_null() { scratch } else { address.cast() })
    }
}

struct DirectStringStorage;

impl crate::vm_string::VmStringStorage for DirectStringStorage {
    type Error = RuntimeError;

    fn set(&self, slots: crate::array_storage::Hp2ArraySlots, count: i32, width: i32, tag: *const c_void) -> RuntimeResult<()> {
        let arrays = array_allocator();
        unsafe { crate::array_storage::set(slots, count, width, tag, arrays) }.map_err(array_error)
    }

    fn shrink(&self, slots: crate::array_storage::Hp2ArraySlots, width: i32, tag: *const c_void) -> RuntimeResult<()> {
        let arrays = array_allocator();
        unsafe { crate::array_storage::shrink(slots, width, tag, arrays) }.map_err(array_error)
    }

    fn checked_int_size(&self, bytes: usize) -> RuntimeResult<i32> {
        let arrays = array_allocator();
        let mut count = 0;
        let status = unsafe { (arrays.checked_int_size)(arrays.context, bytes, &mut count) };
        if status == 0 { Ok(count) } else { Err(array_error(status)) }
    }

    fn release(&self, slots: crate::array_storage::Hp2ArraySlots) -> RuntimeResult<()> {
        let arrays = array_allocator();
        unsafe { crate::array_storage::release(slots, arrays) }.map_err(array_error)
    }
}

fn array_allocator() -> crate::array_storage::Hp2ArrayServices {
    crate::array_storage::system_allocator()
}

fn array_error(status: i32) -> RuntimeError {
    RuntimeError::new("Core VM array operation failed").with_context(&status.to_string())
}

fn assign_string_constant<H: VmHost>(
    _access: CoreAccess<'_>,
    _vm: &VmExecutor<'_, H>,
    unicode: bool,
    frame: *mut hp2_abi::engine::generated::FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    let arrays = array_allocator();
    unsafe { crate::vm_flow::execute_string_constant(unicode, std::ptr::addr_of_mut!((*frame).code), result.cast(), arrays) }
        .map_err(array_error)
}

fn string_argument<H: VmHost>(
    _access: CoreAccess<'_>,
    vm: &VmExecutor<'_, H>,
    frame: *mut hp2_abi::engine::generated::FFrame,
    result: *mut hp2_abi::engine::generated::TArrayRaw,
) -> RuntimeResult<()> {
    // SAFETY: dispatch supplies a live frame and an initialized FString result slot.
    unsafe { vm.step_argument(frame, result.cast()) }
}

fn release_string(
    _access: CoreAccess<'_>,
    value: *mut hp2_abi::engine::generated::TArrayRaw,
) -> RuntimeResult<()> {
    let arrays = array_allocator();
    let slots = unsafe { crate::vm_string::slots(value) };
    unsafe { crate::array_storage::release(slots, arrays) }.map_err(array_error)
}

fn construct_function_values(access: CoreAccess<'_>, function: *mut UFunction, data: *mut u8) -> RuntimeResult<()> {
    let structure = function.cast::<hp2_abi::engine::generated::UStruct>();
    unsafe { crate::object_properties::init_typed_properties(
        data, (*structure).properties_size, std::ptr::null(), 0, 0,
        || (*structure).constructor_link.cast(),
        |property| (*property.cast::<hp2_abi::engine::generated::UProperty>()).offset,
        |property| {
            let property = property.cast::<hp2_abi::engine::generated::UProperty>();
            (*property).element_size.saturating_mul((*property).array_dim)
        },
        |property| (*property.cast::<hp2_abi::engine::generated::UProperty>()).constructor_link_next.cast(),
        |property, destination, source| access.property_value(
            crate::property_value::Hp2ValueOperation::ValueCopyComplete,
            property.cast(), destination, source, 0, None,
        ).map(|_| ()),
        RuntimeError::new,
    ) }
}

fn destroy_function_values(
    access: CoreAccess<'_>, function: *mut UFunction, locals: *mut u8, parameter_values: bool,
) -> RuntimeResult<()> {
    const CPF_PARM: u32 = 0x80;
    let structure = function.cast::<hp2_abi::engine::generated::UStruct>();
    let mut first_error = None;
    let mut property = unsafe { (*structure).constructor_link.cast::<hp2_abi::engine::generated::UProperty>() };
    while !property.is_null() {
        let flags = unsafe { (*property).property_flags };
        if (flags & CPF_PARM != 0) == parameter_values {
            let offset = unsafe { (*property).offset };
            let extent = unsafe { (*property).element_size.checked_mul((*property).array_dim) };
            let size = unsafe { (*structure).properties_size };
            let outcome = if offset < 0 || extent.is_none_or(|width| width < 0 || offset.checked_add(width).is_none_or(|end| end > size)) {
                Err(RuntimeError::new("Invalid function value offset"))
            } else {
                unsafe { access.property_value(
                    crate::property_value::Hp2ValueOperation::ValueDestroy, property,
                    locals.add(offset as usize).cast(), std::ptr::null_mut(), 0, None,
                ) }.map(|_| ())
            };
            if let Err(error) = outcome {
                if first_error.is_none() { first_error = Some(error); }
            }
        }
        property = unsafe { (*property).constructor_link_next.cast() };
    }
    first_error.map_or(Ok(()), Err)
}

fn run_string<const OP: u32, H: VmHost>(
    vm: &VmExecutor<'_, H>,
    frame: *mut hp2_abi::engine::generated::FFrame,
    result: *mut c_void,
) -> RuntimeResult<()> {
    unsafe {
        crate::vm_string::execute_with::<OP, _, RuntimeError>(
            frame,
            result,
            &DirectStringStorage,
            |opcode, object, frame, result| vm.dispatch_operand(opcode, object, frame, result),
        )
    }
}

fn run_eat<H: VmHost>(
    vm: &VmExecutor<'_, H>,
    object: *mut c_void,
    frame: *mut hp2_abi::engine::generated::FFrame,
) -> RuntimeResult<()> {
    unsafe {
        crate::vm_string::eat_with(object, frame, &DirectStringStorage, |opcode, object, frame, result| {
            vm.dispatch_operand(opcode, object, frame, result)
        })
    }
}

fn execute_string<H: VmHost>(
    access: CoreAccess<'_>,
    vm: &VmExecutor<'_, H>,
    operation: crate::vm_owner::VmStringOp,
    opcode: u32,
    object: *mut c_void,
    frame: *mut hp2_abi::engine::generated::FFrame,
    result: *mut c_void,
) -> RuntimeResult<crate::vm_owner::StepControl> {
    string_operation(access, vm, operation, frame, result, object, opcode)
}

fn string_operation<H: VmHost>(
    access: CoreAccess<'_>,
    vm: &VmExecutor<'_, H>,
    operation: crate::vm_owner::VmStringOp,
    frame: *mut hp2_abi::engine::generated::FFrame,
    result: *mut c_void,
    object: *mut c_void,
    opcode: u32,
) -> RuntimeResult<crate::vm_owner::StepControl> {
    use crate::vm_owner::{StepControl, VmStringOp as Op};
    match operation {
        Op::Eat => run_eat(vm, object, frame)?,
        Op::Concat => run_string::<112, _>(vm, frame, result)?,
        Op::At => run_string::<168, _>(vm, frame, result)?,
        Op::Compare => match opcode {
            115 => run_string::<115, _>(vm, frame, result)?,
            116 => run_string::<116, _>(vm, frame, result)?,
            120 => run_string::<120, _>(vm, frame, result)?,
            121 => run_string::<121, _>(vm, frame, result)?,
            122 => run_string::<122, _>(vm, frame, result)?,
            123 => run_string::<123, _>(vm, frame, result)?,
            _ => return Err(RuntimeError::new("Invalid Core string comparison opcode").with_context(&opcode.to_string())),
        },
        Op::CaseInsensitiveEqual => run_string::<124, _>(vm, frame, result)?,
        Op::Len => run_string::<125, _>(vm, frame, result)?,
        Op::InStr => run_string::<126, _>(vm, frame, result)?,
        Op::Mid => run_string::<127, _>(vm, frame, result)?,
        Op::Left => run_string::<128, _>(vm, frame, result)?,
        Op::Right => run_string::<234, _>(vm, frame, result)?,
        Op::Caps => run_string::<235, _>(vm, frame, result)?,
        Op::Chr => run_string::<236, _>(vm, frame, result)?,
        Op::Asc => run_string::<237, _>(vm, frame, result)?,
        Op::Convert => return convert_string(access, vm, opcode, frame, result),
    }
    Ok(StepControl::Continue)
}
struct DirectVariableStorage<'a, 'host, H: VmHost> {
    access: CoreAccess<'a>,
    lvalues: &'a crate::vm_variable::EngineLvalueStorage,
    assignment_sink: *mut u8,
    vm: &'host VmExecutor<'host, H>,
}

impl<H: VmHost> crate::vm_variable::VmVariableStorage for DirectVariableStorage<'_, '_, H> {
    type Error = RuntimeError;

    fn frame_object(&self, frame: *mut c_void) -> *mut c_void {
        unsafe { (*frame.cast::<hp2_abi::engine::generated::FFrame>()).object }
    }

    fn locals(&self, frame: *mut c_void) -> *mut u8 {
        unsafe { (*frame.cast::<hp2_abi::engine::generated::FFrame>()).locals }
    }

    fn resolve(&self, index: i32) -> RuntimeResult<*mut c_void> {
        let object = self.access.object_at(index);
        if object.is_null() && index != -1 {
            Err(RuntimeError::new("Invalid Core VM object index").with_context(&index.to_string()))
        } else {
            Ok(object.cast())
        }
    }

    fn field(&self, property: *mut c_void, field: crate::vm_variable::Hp2VmVariableField) -> i32 {
        use crate::vm_variable::Hp2VmVariableField as F;
        let property = property.cast::<hp2_abi::engine::generated::UProperty>();
        unsafe {
            match field {
                F::VmVariableOffset => (*property).offset,
                F::VmVariableArrayDim => (*property).array_dim,
                F::VmVariableElementSize => (*property).element_size,
                F::VmVariableBitMask => (*property.cast::<hp2_abi::engine::generated::UBoolProperty>()).bit_mask as i32,
                F::VmVariableStructSize => (*property.cast::<hp2_abi::engine::generated::UStruct>()).properties_size,
                F::VmVariableIsBool => i32::from(self.access.record_kind(property.cast()).ok()
                    == Some(crate::core_class_specs::NativeRecordKind::BoolProperty)),
            }
        }
    }

    fn link(&self, property: *mut c_void, link: crate::vm_variable::Hp2VmVariableLink) -> *mut c_void {
        use crate::vm_variable::Hp2VmVariableLink as L;
        let property = property.cast::<hp2_abi::engine::generated::UProperty>();
        unsafe {
            match link {
                L::VmVariableArrayCast => {
                    if self.access.record_kind(property.cast()).ok()
                        == Some(crate::core_class_specs::NativeRecordKind::ArrayProperty) {
                        property.cast()
                    } else {
                        std::ptr::null_mut()
                    }
                }
                L::VmVariableInner => (*property.cast::<hp2_abi::engine::generated::UFixedArrayProperty>()).inner,
                L::VmVariableConstructor => (*property.cast::<hp2_abi::engine::generated::UStruct>()).constructor_link,
                L::VmVariableConstructorNext => (*property).constructor_link_next,
            }
        }
    }

    fn dispatch(&self, frame: *mut c_void, object: *mut c_void, opcode: u8) -> RuntimeResult<()> {
        self.vm.dispatch_operand(opcode, object, frame.cast(), std::ptr::null_mut())
    }

    fn opcode_offset(&self, frame: *mut c_void) -> RuntimeResult<i32> {
        let frame = frame.cast::<hp2_abi::engine::generated::FFrame>();
        let node = unsafe { (*frame).node.cast::<hp2_abi::engine::generated::UStruct>() };
        let base = unsafe { (*node).script.data.cast::<u8>() };
        let code = unsafe { (*frame).code };
        if base.is_null() || code.is_null() {
            return Err(RuntimeError::new("Core VM frame has no script cursor"));
        }
        Ok(unsafe { code.offset_from(base) as i32 })
    }

    fn trace(&self, _: *mut c_void, _: *mut c_void, _: crate::vm_variable::Hp2VmVariableOp, _: i32) -> RuntimeResult<()> {
        Ok(())
    }

    fn copy(&self, property: *mut c_void, destination: *mut c_void, source: *mut u8,
        mode: crate::vm_variable::Hp2VmVariableCopy) -> RuntimeResult<()> {
        use crate::property_value::Hp2ValueOperation as V;
        let operation = match mode {
            crate::vm_variable::Hp2VmVariableCopy::VmVariableCopySingle => V::ValueCopySingle,
            crate::vm_variable::Hp2VmVariableCopy::VmVariableCopyComplete
            | crate::vm_variable::Hp2VmVariableCopy::VmVariableCopyMember => V::ValueCopyComplete,
        };
        unsafe { self.access.property_value(operation, property.cast(), destination, source.cast(), 0, None)?; }
        Ok(())
    }

    fn warning(&self, frame: *mut c_void, warning: crate::vm_variable::Hp2VmVariableWarning, a: i32, b: i32) -> RuntimeResult<()> {
        use crate::vm_variable::Hp2VmVariableWarning as W;
        let mut text: Vec<u32> = match warning {
            W::VmVariableNullClass => "Accessed null class context",
            W::VmVariableNullContext => "Accessed None: ",
            W::VmVariableBounds => "Accessed array out of bounds (",
            W::VmVariableNullAssignment => "Attempt to assigned variable through None",
        }.chars().map(u32::from).collect();
        if matches!(warning, W::VmVariableNullContext) {
            let property = unsafe { self.lvalues.property_slot().read().cast::<UObject>() };
            if property.is_null() {
                text.extend("Unknown".chars().map(u32::from));
            } else {
                text.extend(self.access.name_text(unsafe { i32::from_ne_bytes((*property).name) })?);
            }
        } else if matches!(warning, W::VmVariableBounds) {
            text.extend(formatted_vm_number(|buffer| crate::ansi::format_int(a, buffer)));
            text.push(u32::from(b'/'));
            text.extend(formatted_vm_number(|buffer| crate::ansi::format_int(b, buffer)));
            text.push(u32::from(b')'));
        }
        let event = if matches!(warning, W::VmVariableNullAssignment) { "ScriptWarning" } else { "Log" };
        log_frame(self.access, frame.cast(), event, &text)
    }

    fn default_address(&self, object: *mut c_void, offset: i32) -> RuntimeResult<*mut u8> {
        if offset < 0 { return Err(RuntimeError::new("Negative class-default property offset")); }
        let class = unsafe { (*object.cast::<UObject>()).class.cast::<hp2_abi::engine::generated::UClass>() };
        let defaults = unsafe { self.access.default_object(class)? };
        Ok(defaults.cast::<u8>().wrapping_add(offset as usize))
    }

    fn default_object(&self, class: *mut c_void) -> RuntimeResult<*mut c_void> {
        unsafe { self.access.default_object(class.cast()) }.map(|object| object.cast())
    }

    fn array_num(&self, array: *mut c_void) -> i32 {
        unsafe { (*array.cast::<hp2_abi::engine::generated::TArrayRaw>()).num }
    }

    fn array_data(&self, array: *mut c_void) -> *mut u8 {
        unsafe { (*array.cast::<hp2_abi::engine::generated::TArrayRaw>()).data.cast() }
    }

    fn array_add(&self, array: *mut c_void, element_size: i32, count: i32) -> RuntimeResult<()> {
        if element_size <= 0 || count < 0 { return Err(RuntimeError::new("Invalid dynamic array growth")); }
        let slots = unsafe { crate::vm_string::slots(array.cast()) };
        let arrays = array_allocator();
        unsafe { crate::array_storage::add(slots, count, element_size, b"CoreVmArray\0".as_ptr().cast(), arrays) }
            .map(|_| ()).map_err(array_error)
    }

    fn struct_outer(&self, property: *mut c_void) -> RuntimeResult<*mut c_void> {
        use crate::core_class_specs::NativeRecordKind as K;
        let structure = unsafe { (*property.cast::<UObject>()).outer };
        if structure.is_null() || !matches!(
            unsafe { self.access.record_kind(structure.cast())? },
            K::Struct | K::Class | K::State | K::Function,
        ) {
            Err(RuntimeError::new("Struct member owner is not UStruct"))
        } else {
            Ok(structure)
        }
    }

    fn with_scratch(
        &self,
        _: &crate::vm_variable::Hp2VmVariableExecution,
        _: crate::vm_variable::Hp2VmVariableOp,
        _: *mut c_void,
        _: *mut c_void,
        size: usize,
        second_size: usize,
        _: *mut c_void,
        run: &mut dyn FnMut(*mut u8, *mut u8) -> RuntimeResult<()>,
    ) -> RuntimeResult<()> {
        let first_layout = std::alloc::Layout::from_size_align(size.max(1), 16)
            .map_err(|_| RuntimeError::new("Invalid Core VM scratch layout"))?;
        let second_layout = std::alloc::Layout::from_size_align(second_size.max(1), 16)
            .map_err(|_| RuntimeError::new("Invalid Core VM scratch layout"))?;
        let first = std::ptr::NonNull::new(unsafe { std::alloc::alloc(first_layout) })
            .ok_or_else(|| RuntimeError::new("Unable to allocate Core VM scratch"))?;
        let Some(second) = std::ptr::NonNull::new(unsafe { std::alloc::alloc(second_layout) }) else {
            unsafe { std::alloc::dealloc(first.as_ptr(), first_layout) };
            return Err(RuntimeError::new("Unable to allocate Core VM scratch"));
        };
        unsafe {
            first.as_ptr().write_bytes(0, first_layout.size());
            second.as_ptr().write_bytes(0, second_layout.size());
        }
        let result = run(first.as_ptr(), second.as_ptr());
        unsafe {
            std::alloc::dealloc(second.as_ptr(), second_layout);
            std::alloc::dealloc(first.as_ptr(), first_layout);
        }
        result
    }

    fn destroy(&self, property: *mut c_void, address: *mut u8) -> RuntimeResult<()> {
        unsafe {
            self.access.property_value(crate::property_value::Hp2ValueOperation::ValueDestroy,
                property.cast(), address.cast(), std::ptr::null_mut(), 0, None)?;
        }
        Ok(())
    }

    fn compare(&self, structure: *mut c_void, first: *mut u8, second: *mut u8) -> RuntimeResult<u32> {
        let equal = unsafe {
            self.access.property_value(crate::property_value::Hp2ValueOperation::ValueIdentical,
                structure.cast(), first.cast(), second.cast(), 0, None)?
        };
        Ok(equal as u32)
    }

    fn invalid_bool(&self) -> RuntimeResult<()> {
        self.access.emit_warning(&"Invalid bool property value".chars().map(u32::from).collect::<Vec<_>>())
    }

    fn assignment_sink(&self) -> (*mut u8, usize) { (self.assignment_sink, 16) }
}

fn variable_operation(operation: crate::vm_owner::VmPropertyOp) -> crate::vm_variable::Hp2VmVariableOp {
    use crate::vm_owner::VmPropertyOp as P;
    use crate::vm_variable::Hp2VmVariableOp as V;
    match operation {
        P::LocalVariable => V::VmVariableLocal,
        P::InstanceVariable => V::VmVariableInstance,
        P::DefaultVariable => V::VmVariableDefault,
        P::ClassContext => V::VmVariableClassContext,
        P::ArrayElement => V::VmVariableArrayElement,
        P::DynamicArrayElement => V::VmVariableDynArrayElement,
        P::DynamicArrayCount => V::VmVariableDynArrayCount,
        P::BoolVariable => V::VmVariableBool,
        P::StructMember => V::VmVariableStructMember,
        P::NativeParameter => V::VmVariableNativeParm,
        P::Let => V::VmVariableLet,
        P::LetBool => V::VmVariableLetBool,
        P::SelfObject => V::VmVariableSelf,
        P::Context => V::VmVariableContext,
        P::StructEqual => V::VmVariableStructCmpEq,
        P::StructNotEqual => V::VmVariableStructCmpNe,
    }
}

fn execute_property<H: VmHost>(
    access: CoreAccess<'_>,
    lvalues: &crate::vm_variable::EngineLvalueStorage,
    assignment_sink: &std::cell::UnsafeCell<[u8; 16]>,
    vm: &VmExecutor<'_, H>,
    operation: crate::vm_owner::VmPropertyOp,
    object: *mut c_void,
    frame: *mut hp2_abi::engine::generated::FFrame,
    result: *mut c_void,
) -> RuntimeResult<crate::vm_owner::StepControl> {
    execute_property_with(access, lvalues, assignment_sink, vm, operation, object, frame, result,
        &mut |frame, object, result| {
            // SAFETY: the canonical variable owner supplies a live operand.
            unsafe { crate::vm_frame::step_with(frame.cast(), object, result,
                |opcode, object, frame, result| vm.dispatch_operand(opcode, object, frame, result)) }
        })
}

fn execute_property_with<H: VmHost>(
    access: CoreAccess<'_>,
    lvalues: &crate::vm_variable::EngineLvalueStorage,
    assignment_sink: &std::cell::UnsafeCell<[u8; 16]>,
    vm: &VmExecutor<'_, H>,
    operation: crate::vm_owner::VmPropertyOp,
    object: *mut c_void,
    frame: *mut FFrame,
    result: *mut c_void,
    step: &mut impl FnMut(*mut c_void, *mut c_void, *mut c_void) -> RuntimeResult<()>,
) -> RuntimeResult<crate::vm_owner::StepControl> {
    let storage = DirectVariableStorage {
        access,
        lvalues,
        assignment_sink: assignment_sink.get().cast(),
        vm,
    };
    let execution = crate::vm_variable::Hp2VmVariableExecution {
        frame: frame.cast(),
        object,
        code: unsafe { std::ptr::addr_of_mut!((*frame).code) },
        property: lvalues.property_slot().cast(),
        address: lvalues.address_slot(),
    };
    unsafe {
        crate::vm_variable::execute_with(variable_operation(operation), &execution, result, &storage, step)?;
    }
    Ok(crate::vm_owner::StepControl::Continue)
}

fn execute_call<H: VmHost>(
    access: CoreAccess<'_>,
    vm: &VmExecutor<'_, H>,
    operation: crate::vm_owner::VmCallOp,
    object: *mut c_void,
    frame: *mut FFrame,
    result: *mut c_void,
) -> RuntimeResult<crate::vm_owner::StepControl> {
    execute_call_with(access, vm, operation, object, frame, result,
        &|vm, _function, parent, _property, destination| {
            // SAFETY: the canonical call owner supplies its current argument.
            unsafe { vm.step_argument(parent, destination) }
        })
}

fn execute_call_with<H: VmHost>(
    access: CoreAccess<'_>,
    vm: &VmExecutor<'_, H>,
    operation: crate::vm_owner::VmCallOp,
    object: *mut c_void,
    frame: *mut FFrame,
    result: *mut c_void,
    evaluate: &impl Fn(&VmExecutor<'_, H>, *mut UFunction, *mut FFrame, *mut hp2_abi::engine::generated::UProperty, *mut c_void) -> RuntimeResult<()>,
) -> RuntimeResult<crate::vm_owner::StepControl> {
    use crate::vm_owner::{StepControl, VmCallOp};
    let (function, reference) = unsafe {
        match operation {
            VmCallOp::Virtual => {
                let name = crate::vm_bytecode::read::<i32>(std::ptr::addr_of_mut!((*frame).code));
                (access.find_object_function_by_name(object.cast(), name, false)?, name)
            }
            VmCallOp::Final => {
                let index = crate::vm_bytecode::read::<i32>(std::ptr::addr_of_mut!((*frame).code));
                (access.object_at(index).cast::<UFunction>(), index)
            }
            VmCallOp::Global => {
                let name = crate::vm_bytecode::read::<i32>(std::ptr::addr_of_mut!((*frame).code));
                (access.find_object_function_by_name(object.cast(), name, true)?, name)
            }
            // Source execIterator is only a marker. Its following native expression
            // evaluates arguments once and owns PRE_ITERATOR/POST_ITERATOR control.
            VmCallOp::Iterator => return Ok(StepControl::Continue),
        }
    };
    if crate::debug_trace::enabled() {
        crate::debug_trace::vm_call(access, object, function.cast());
    }
    if function.is_null() {
        let mut message: Vec<u32> = "Core VM function lookup failed: ".chars().map(u32::from).collect();
        message.extend(object_path_text(vm, object.cast())?);
        message.extend(" -> ".chars().map(u32::from));
        if matches!(operation, VmCallOp::Final) {
            message.extend(format!("object index {reference}").chars().map(u32::from));
        } else {
            message.extend(vm.host().name_text(reference)?);
        }
        return Err(RuntimeError::from_utf32(message));
    }
    call_function(access, vm, object, function, frame, result, evaluate)?;
    Ok(StepControl::Continue)
}

fn call_function<H: VmHost>(
    access: CoreAccess<'_>,
    vm: &VmExecutor<'_, H>,
    object: *mut c_void,
    function: *mut UFunction,
    parent: *mut hp2_abi::engine::generated::FFrame,
    result: *mut c_void,
    evaluate: &impl Fn(&VmExecutor<'_, H>, *mut UFunction, *mut FFrame, *mut hp2_abi::engine::generated::UProperty, *mut c_void) -> RuntimeResult<()>,
) -> RuntimeResult<()> {
    use crate::object_owner::OwnedFunctionRecord;
    let flags = unsafe { (*function).function_flags };
    if flags & crate::vm_dispatch::FUNC_NATIVE != 0 {
        let native = unsafe { OwnedFunctionRecord::binding(function.cast())? }
            .ok_or_else(|| {
                let function_name = self_name_text(access, i32::from_ne_bytes(unsafe { (*function.cast::<hp2_abi::engine::generated::UObject>()).name }));
                let owner = unsafe { (*function.cast::<hp2_abi::engine::generated::UObject>()).outer };
                let owner_name = if owner.is_null() { String::new() } else {
                    self_name_text(access, i32::from_ne_bytes(unsafe { (*owner.cast::<hp2_abi::engine::generated::UObject>()).name }))
                };
                RuntimeError::new(&format!("Core native function has no Rust binding: {owner_name}.{function_name}"))
            })?;
        return unsafe { vm.execute_native(native, u32::from((*function).i_native), object, parent, result) }.map(|_| ());
    }

    let structure = function.cast::<hp2_abi::engine::generated::UStruct>();
    let size = unsafe { (*function).parms_size as usize };
    let raw_size = unsafe { (*structure).properties_size };
    let raw_align = unsafe { (*structure).properties_align };
    let storage_size = usize::try_from(raw_size).map_err(|_| RuntimeError::new("Invalid function locals size"))?;
    let storage_align = usize::try_from(raw_align).ok()
        .filter(|value| value.is_power_of_two()).unwrap_or(1);
    if size > storage_size {
        return Err(RuntimeError::new("Function parameters exceed locals storage"));
    }
    let layout = std::alloc::Layout::from_size_align(storage_size.max(1), storage_align)
        .map_err(|_| RuntimeError::new("Invalid function parameter layout"))?;
    let params = std::ptr::NonNull::new(unsafe { std::alloc::alloc(layout) })
        .ok_or_else(|| RuntimeError::new("Unable to allocate function parameters"))?;
    if let Err(error) = construct_function_values(access, function, params.as_ptr()) {
        unsafe { std::alloc::dealloc(params.as_ptr(), layout); }
        return Err(error.with_context("constructing call parameters"));
    }

    let mut outputs = Vec::new();
    let mut field = unsafe { (*structure).children.cast::<UField>() };
    let outcome: RuntimeResult<()> = (|| {
        while !field.is_null() {
            let property = field.cast::<hp2_abi::engine::generated::UProperty>();
            let property_flags = unsafe { (*property).property_flags };
            if property_flags & 0x80 != 0 && property_flags & 0x400 == 0 {
                let offset = usize::try_from(unsafe { (*property).offset })
                    .map_err(|_| RuntimeError::new("Invalid function parameter offset"))?;
                let destination = unsafe { params.as_ptr().add(offset) };
                if property_flags & 0x100 != 0 {
                    let lvalue = vm.host().reference_argument(vm, parent, destination.cast(), unsafe { (*property).element_size as usize })?;
                    if lvalue.is_null() {
                        return Err(RuntimeError::new("Function out parameter is not an lvalue"));
                    }
                    if property_flags & 0x2000 == 0 {
                        unsafe { access.property_value(crate::property_value::Hp2ValueOperation::ValueCopyComplete, property, destination.cast(), lvalue.cast(), 0, None)?; }
                    }
                    outputs.push((property, lvalue, destination));
                } else {
                    evaluate(vm, function, parent, property, destination.cast())?;
                }
            }
            field = unsafe { (*field).next.cast() };
        }
        unsafe { vm.finish_arguments(parent)?; }
        let call_result = unsafe {
            vm.execute_event(object, function, parent, params.as_ptr(), result)
        };
        call_result?;
        for (property, destination, source) in outputs {
            unsafe {
                access.property_value(
                    crate::property_value::Hp2ValueOperation::ValueCopyComplete,
                    property,
                    destination.cast(),
                    source.cast(),
                    0,
                    None,
                )?;
            }
        }
        Ok(())
    })();

    let teardown = destroy_function_values(access, function, params.as_ptr(), true);
    unsafe { std::alloc::dealloc(params.as_ptr(), layout); }
    match (outcome, teardown) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn evaluate_string<H: VmHost>(
    vm: &VmExecutor<'_, H>,
    frame: *mut hp2_abi::engine::generated::FFrame,
) -> RuntimeResult<Vec<u32>> {
    let mut value = hp2_abi::engine::generated::TArrayRaw {
        data: std::ptr::null_mut(), num: 0, max: 0,
    };
    let evaluated = vm.host().string_argument(vm, frame, &mut value);
    if let Err(error) = evaluated {
        let cleanup = vm.host().release_string(&mut value);
        return match cleanup { Err(cleanup) => Err(error.with_context(&cleanup.to_string())), Ok(()) => Err(error) };
    }
    let units = unsafe {
        let n = (*std::ptr::addr_of!(value)).num.max(0) as usize;
        let data = (*std::ptr::addr_of!(value)).data.cast::<crate::ansi::Tchar>();
        if n == 0 || data.is_null() {
            Vec::new()
        } else {
            std::slice::from_raw_parts(data, n).iter().copied()
                .take_while(|&unit| unit != 0).map(|unit| unit as u32).collect()
        }
    };
    vm.host().release_string(&mut value)?;
    Ok(units)
}

fn write_string(
    value: *mut hp2_abi::engine::generated::TArrayRaw,
    text: &[u32],
) -> RuntimeResult<()> {
    let arrays = array_allocator();
    let len = text.iter().position(|unit| *unit == 0).unwrap_or(text.len());
    let count = len.checked_add(1).ok_or_else(|| RuntimeError::new("Core VM string exceeds INT size"))?;
    let slots = unsafe { crate::vm_string::slots(value) };
    unsafe {
        crate::array_storage::set(
            slots,
            i32::try_from(count).map_err(|_| RuntimeError::new("Core VM string exceeds INT size"))?,
            std::mem::size_of::<crate::ansi::Tchar>() as i32,
            b"FString\0".as_ptr().cast(),
            arrays,
        )
    }.map_err(array_error)?;
    // Every caller supplies a separate UTF-32 snapshot, never a view of `value`.
    // TCHAR and u32 have the same four-byte representation on supported platforms.
    unsafe {
        let destination = (*slots.data).cast::<u32>();
        destination.copy_from_nonoverlapping(text.as_ptr(), len);
        destination.add(len).write(0);
    }
    unsafe {
        crate::array_storage::shrink(
            slots,
            std::mem::size_of::<crate::ansi::Tchar>() as i32,
            b"FString\0".as_ptr().cast(),
            arrays,
        )
    }.map_err(array_error)
}
fn formatted_vm_number(
    format: impl FnOnce(&mut [std::mem::MaybeUninit<crate::ansi::Tchar>; 512]),
) -> Vec<u32> {
    let mut buffer = [std::mem::MaybeUninit::uninit(); 512];
    format(&mut buffer);
    let mut output = Vec::new();
    for slot in &buffer {
        // SAFETY: the canonical formatter always writes a terminated TCHAR string.
        let unit = unsafe { slot.assume_init() };
        if unit == 0 { break; }
        output.push(unit as u32);
    }
    output
}

fn formatted_vm_vector(value: hp2_abi::FVector) -> Vec<u32> {
    let mut output = formatted_vm_number(|buffer| crate::ansi::format_float(value.x, buffer));
    output.push(u32::from(b','));
    output.extend(formatted_vm_number(|buffer| crate::ansi::format_float(value.y, buffer)));
    output.push(u32::from(b','));
    output.extend(formatted_vm_number(|buffer| crate::ansi::format_float(value.z, buffer)));
    output
}

fn formatted_vm_rotator(value: crate::math::FRotator) -> Vec<u32> {
    let mut output = formatted_vm_number(|buffer| crate::ansi::format_int(value.pitch & 65535, buffer));
    output.push(u32::from(b','));
    output.extend(formatted_vm_number(|buffer| crate::ansi::format_int(value.yaw & 65535, buffer)));
    output.push(u32::from(b','));
    output.extend(formatted_vm_number(|buffer| crate::ansi::format_int(value.roll & 65535, buffer)));
    output
}

fn self_name_text(access: CoreAccess<'_>, name: i32) -> String {
    access
        .name_text(name)
        .map(|units| {
            let mut text = String::new();
            for index in 0..units.len() {
                if units[index] == 0 { break; }
                if let Some(unit) = char::from_u32(units[index]) { text.push(unit); }
            }
            text
        })
        .unwrap_or_default()
}

fn object_path_text<H: VmHost>(vm: &VmExecutor<'_, H>, object: *mut UObject) -> RuntimeResult<Vec<u32>> {
    crate::core_reflection::object_path_text_with(object, &|name| vm.host().name_text(name))
}
fn convert_string<H: VmHost>(
    access: CoreAccess<'_>,
    vm: &VmExecutor<'_, H>,
    opcode: u32,
    frame: *mut hp2_abi::engine::generated::FFrame,
    result: *mut c_void,
) -> RuntimeResult<crate::vm_owner::StepControl> {
    let spec = core_native_catalog::by_opcode(opcode as i32)
        .ok_or_else(|| RuntimeError::new("Core string conversion has no catalog identity").with_context(&opcode.to_string()))?;
    let name = spec.token_name;
    let units = if name.starts_with("intUObjectexecStringTo") {
        evaluate_string(vm, frame)?
    } else {
        Vec::new()
    };
    let mut text: Vec<crate::ansi::Tchar> = units.iter().map(|&unit| unit as crate::ansi::Tchar).collect();
    text.push(0);
    let ansi = if name.starts_with("intUObjectexecStringTo") {
        Some(match unsafe { crate::ansi::published_services() } {
            Ok(services) => services,
            Err(_) => {
                // SAFETY: this direct Rust-only VM path runs serialized on the engine thread.
                unsafe { crate::ansi::initialize_os_services(); }
                unsafe { crate::ansi::published_services() }
                    .map_err(|status| RuntimeError::new("Core ANSI conversion services are unavailable").with_context(&status.to_string()))?
            }
        })
    } else {
        None
    };
    let parse_ansi = || ansi.ok_or_else(|| RuntimeError::new("Missing Core ANSI conversion services"));
    match name {
        "intUObjectexecStringToInt" => {
            let value = unsafe { crate::vm_convert::parse_string_to_int(text.as_ptr(), parse_ansi()?) }
                .map_err(|status| RuntimeError::new("Core string-to-int conversion failed").with_context(&status.to_string()))?;
            unsafe { result.cast::<i32>().write(value); }
        }
        "intUObjectexecStringToByte" => {
            let value = unsafe { crate::vm_convert::parse_string_to_int(text.as_ptr(), parse_ansi()?) }
                .map_err(|status| RuntimeError::new("Core string-to-byte conversion failed").with_context(&status.to_string()))?;
            unsafe { result.cast::<u8>().write(value as u8); }
        }
        "intUObjectexecStringToFloat" => {
            let value = unsafe { crate::vm_convert::parse_string_to_float(text.as_ptr(), parse_ansi()?) }
                .map_err(|status| RuntimeError::new("Core string-to-float conversion failed").with_context(&status.to_string()))?;
            unsafe { result.cast::<f32>().write(value); }
        }
        "intUObjectexecStringToBool" => {
            let value = unsafe { crate::vm_convert::parse_string_to_bool(text.as_ptr(), parse_ansi()?) }
                .map_err(|status| RuntimeError::new("Core string-to-bool conversion failed").with_context(&status.to_string()))?;
            unsafe { result.cast::<i32>().write(value); }
        }
        "intUObjectexecStringToName" => {
            let value = access.find_name(&units)?;
            unsafe { result.cast::<i32>().write(value); }
        }
        "intUObjectexecNameToString" => {
            let mut value = 0_i32;
            unsafe { vm.step_argument(frame, (&raw mut value).cast())?; }
            let text = vm.host().name_text(value)?;
            write_string(result.cast(), &text)?;
        }
        "intUObjectexecByteToString" => {
            let mut value = 0_u8;
            unsafe { vm.step_argument(frame, (&raw mut value).cast())?; }
            let text = formatted_vm_number(|buffer| crate::ansi::format_int(i32::from(value), buffer));
            write_string(result.cast(), &text)?;
        }
        "intUObjectexecIntToString" => {
            let mut value = 0_i32;
            unsafe { vm.step_argument(frame, (&raw mut value).cast())?; }
            let text = formatted_vm_number(|buffer| crate::ansi::format_int(value, buffer));
            write_string(result.cast(), &text)?;
        }
        "intUObjectexecFloatToString" => {
            let mut value = 0_f32;
            unsafe { vm.step_argument(frame, (&raw mut value).cast())?; }
            let text = formatted_vm_number(|buffer| crate::ansi::format_float(value, buffer));
            write_string(result.cast(), &text)?;
        }
        "intUObjectexecBoolToString" => {
            let mut value = 0_u32;
            unsafe { vm.step_argument(frame, (&raw mut value).cast())?; }
            let text = unsafe {
                let source = if value != 0 {
                    (&raw const crate::vm_convert::GTrue).cast::<crate::ansi::Tchar>()
                } else {
                    (&raw const crate::vm_convert::GFalse).cast::<crate::ansi::Tchar>()
                };
                let mut output = Vec::new();
                for offset in 0..64 {
                    let unit = source.add(offset).read();
                    if unit == 0 { break; }
                    output.push(unit as u32);
                }
                output
            };
            write_string(result.cast(), &text)?;
        }
        "intUObjectexecVectorToString" => {
            let mut value = hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 };
            unsafe { vm.step_argument(frame, (&raw mut value).cast())?; }
            write_string(result.cast(), &formatted_vm_vector(value))?;
        }
        "intUObjectexecRotatorToString" => {
            let mut value = crate::math::FRotator { pitch: 0, yaw: 0, roll: 0 };
            unsafe { vm.step_argument(frame, (&raw mut value).cast())?; }
            write_string(result.cast(), &formatted_vm_rotator(value))?;
        }
        "intUObjectexecVectorToRotator" => {
            let mut value = hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 };
            unsafe { vm.step_argument(frame, (&raw mut value).cast())?; }
            unsafe { result.cast::<crate::math::FRotator>().write(crate::math::vector_rotation(value)); }
        }
        "intUObjectexecRotatorToVector" => {
            let mut value = crate::math::FRotator { pitch: 0, yaw: 0, roll: 0 };
            unsafe { vm.step_argument(frame, (&raw mut value).cast())?; }
            unsafe { result.cast::<hp2_abi::FVector>().write(crate::math::rotator_vector(value, crate::math::trig_table())); }
        }
        "intUObjectexecStringToVector" => {
            let value = unsafe { crate::vm_convert::parse_string_to_vector(text.as_ptr(), parse_ansi()?) }
                .map_err(|status| RuntimeError::new("Core string-to-vector conversion failed").with_context(&status.to_string()))?;
            unsafe { result.cast::<hp2_abi::FVector>().write(value); }
        }
        "intUObjectexecStringToRotator" => {
            let value = unsafe { crate::vm_convert::parse_string_to_rotator(text.as_ptr(), parse_ansi()?) }
                .map_err(|status| RuntimeError::new("Core string-to-rotator conversion failed").with_context(&status.to_string()))?;
            unsafe { result.cast::<crate::math::FRotator>().write(value); }
        }
        "intUObjectexecObjectToString" => {
            let mut object = std::ptr::null_mut::<UObject>();
            unsafe { vm.step_argument(frame, (&raw mut object).cast())?; }
            let text = object_path_text(vm, object)?;
            write_string(result.cast(), &text)?;
        }
        _ => return Err(RuntimeError::new("Unsupported Core string conversion identity").with_context(name)),
    }
    Ok(crate::vm_owner::StepControl::Continue)
}

struct DirectTraceScope(*mut crate::ansi_trace::Context);

impl Drop for DirectTraceScope {
    fn drop(&mut self) {
        // SAFETY: this handle is returned by the matching direct scope begin.
        unsafe { crate::ansi_trace::context_scope_end(self.0) };
    }
}

fn trace_path<H: VmHost>(
    vm: &VmExecutor<'_, H>,
    object: *mut UObject,
) -> RuntimeResult<Vec<u8>> {
    let mut text = object_path_text(vm, object)?;
    text.push(0);
    // SAFETY: the text is NUL-terminated and the returned ANSI ring slot is copied immediately.
    let ansi = unsafe { crate::ansi::to_ansi_string(text.as_ptr().cast()) };
    if ansi.is_null() {
        return Err(RuntimeError::new("Core RNG trace path conversion failed"));
    }
    // SAFETY: `to_ansi_string` returns a NUL-terminated static-ring string.
    Ok(unsafe { std::ffi::CStr::from_ptr(ansi.cast()) }
        .to_bytes()
        .to_vec())
}
fn begin_rng_trace<H: VmHost>(
    access: CoreAccess<'_>,
    vm: &VmExecutor<'_, H>,
    operation: crate::vm_owner::VmSystemOp,
    native_slot: u32,
    object: *mut c_void,
    frame: *mut hp2_abi::engine::generated::FFrame,
) -> RuntimeResult<*mut crate::ansi_trace::Context> {
    use crate::vm_owner::VmSystemOp as Op;
    let native = match operation {
        Op::Rand => c"Core.Object.Rand",
        Op::FRand => c"Core.Object.FRand",
        Op::VRand => c"Core.Object.VRand",
        Op::RotRand => c"Core.Object.RotRand",
        Op::RandRange => c"Core.Object.RandRange",
        _ => return Ok(std::ptr::null_mut()),
    };
    if !access.vm_trace_enabled() {
        return Ok(std::ptr::null_mut());
    }
    let native_slot = i32::try_from(native_slot)
        .map_err(|_| RuntimeError::new("Invalid Core RNG native slot"))?;
    let actor = object.cast::<UObject>();
    let class = if actor.is_null() {
        std::ptr::null_mut()
    } else {
        unsafe { (*actor).class.cast::<UObject>() }
    };
    let function = unsafe { (*frame).node.cast::<UObject>() };
    let actor_path = trace_path(vm, actor)?;
    let class_path = trace_path(vm, class)?;
    let function_path = trace_path(vm, function)?;
    let script = if function.is_null() {
        std::ptr::null_mut()
    } else {
        unsafe {
            (*function.cast::<hp2_abi::engine::generated::UStruct>())
                .script
                .data
                .cast::<u8>()
        }
    };
    let code = unsafe { (*frame).code };
    let offset = if function.is_null() {
        -1
    } else {
        (code as usize)
            .checked_sub(script as usize)
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| RuntimeError::new("Invalid Core RNG trace callsite"))?
    };
    Ok(crate::ansi_trace::with_process_state(|trace| {
        crate::ansi_trace::direct_native_scope_begin(
            trace,
            actor.cast(),
            function.cast(),
            native,
            native_slot,
            offset,
            actor_path,
            class_path,
            function_path,
        )
    }))
}

fn execute_system<H: VmHost>(
    access: CoreAccess<'_>,
    vm: &VmExecutor<'_, H>,
    operation: crate::vm_owner::VmSystemOp,
    native_slot: u32,
    object: *mut c_void,
    frame: *mut hp2_abi::engine::generated::FFrame,
    result: *mut c_void,
) -> RuntimeResult<crate::vm_owner::StepControl> {
    use crate::vm_owner::{StepControl, VmSystemOp as Op};
    let _trace_scope = DirectTraceScope(begin_rng_trace(
        access, vm, operation, native_slot, object, frame,
    )?);
    match operation {
        Op::Log | Op::Warn => {
            let mut value = hp2_abi::engine::generated::TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
            let outcome = (|| {
                vm.host().string_argument(vm, frame, &mut value)?;
                let mut category = access.intern_str("ScriptLog", crate::name_owner::FindName::Find)?;
                if operation == Op::Log {
                    unsafe { vm.step_argument(frame, (&raw mut category).cast())?; }
                }
                unsafe { vm.finish_arguments(frame)?; }
                let text = if value.num == 0 { &[][..] } else {
                    // The evaluated FString owns its buffer independently of canonical records.
                    let text = unsafe { std::slice::from_raw_parts(value.data.cast::<u32>(), value.num as usize) };
                    &text[..text.iter().position(|unit| *unit == 0).unwrap_or(text.len())]
                };
                if operation == Op::Warn {
                    log_frame(access, frame, "Log", text)
                } else if access.name_suppressed(category) {
                    Ok(())
                } else {
                    vm.host().log(category, text)
                }
            })();
            let cleanup = vm.host().release_string(&mut value);
            outcome.and(cleanup)?;
        }
        Op::CommandletMain => {
            let mut value = hp2_abi::engine::generated::TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
            let outcome = (|| {
                vm.host().string_argument(vm, frame, &mut value)?;
                unsafe { vm.finish_arguments(frame)?; }
                let returned = unsafe { commandlet_main_owned(access, object.cast(), &mut value)? };
                unsafe { result.cast::<i32>().write(returned); }
                Ok(())
            })();
            let cleanup = vm.host().release_string(&mut value);
            outcome.and(cleanup)?;
        }
        Op::SaveConfig => {
            // SAFETY: this native runs with a live frame and a canonical object.
            unsafe { vm.finish_arguments(frame)?; }
            unsafe {
                access.save_config(object.cast::<UObject>(), crate::object_config::CPF_CONFIG_FLAG, None)
            }?;
        }
        Op::StaticSaveConfig => {
            // SAFETY: this native runs with a live frame and a canonical object.
            unsafe { vm.finish_arguments(frame)?; }
            let class = unsafe { (*object.cast::<UObject>()).class.cast::<hp2_abi::engine::generated::UClass>() };
            let defaults = unsafe { access.default_object(class)? };
            unsafe { access.save_config(defaults, crate::object_config::CPF_CONFIG_FLAG, None) }?;
        }
        Op::GetPropertyText | Op::SetPropertyText => {
            let property_name = evaluate_string(vm, frame)?;
            let property_value = if operation == Op::SetPropertyText {
                Some(evaluate_string(vm, frame)?)
            } else {
                None
            };
            unsafe { vm.finish_arguments(frame)?; }
            let name = access.find_name(&property_name)?;
            let class = unsafe { (*object.cast::<UObject>()).class.cast::<hp2_abi::engine::generated::UClass>() };
            let field_class = access.native_class("Property")?;
            let property = unsafe { access.find_field(class.cast(), name, field_class).cast::<hp2_abi::engine::generated::UProperty>() };
            if property.is_null() {
                if operation == Op::GetPropertyText {
                    write_string(result.cast(), &[])?;
                }
                return Ok(StepControl::Continue);
            }
            let defaults = unsafe { access.default_object(class)? };
            if operation == Op::GetPropertyText {
                let text = unsafe { access.property_export_text(property, object.cast(), defaults, 0)? };
                write_string(result.cast(), &text)?;
            } else if let Some(value) = property_value {
                unsafe { access.property_import_text(property, object.cast(), &value, defaults, 0)?; }
            }
        }
        Op::DynamicCast | Op::MetaCast => {
            let index = unsafe { crate::vm_bytecode::read::<i32>(std::ptr::addr_of_mut!((*frame).code)) };
            let class = vm.host().object_at(index)?;
            let mut candidate = std::ptr::null_mut::<UObject>();
            unsafe {
                vm.step_argument(frame, (&raw mut candidate).cast())?;
                crate::vm_convert::cast_with(
                    candidate.cast(), class, operation == Op::MetaCast, result.cast(),
                    |object| {
                        Ok::<_, RuntimeError>(crate::object_model::is_a(
                            object, access.native_class("Class")?.cast(),
                        ))
                    },
                )?;
            }
        }
        Op::ClassIsChildOf => {
            let mut child = std::ptr::null_mut::<UObject>();
            let mut parent = std::ptr::null_mut::<UObject>();
            unsafe {
                vm.step_argument(frame, (&raw mut child).cast())?;
                vm.step_argument(frame, (&raw mut parent).cast())?;
                vm.finish_arguments(frame)?;
                result.cast::<u32>().write(u32::from(
                    !child.is_null() && !parent.is_null() && crate::object_model::is_child_of(child.cast(), parent.cast()),
                ));
            }
        }
        Op::IsA => {
            let mut class_name = 0_i32;
            unsafe {
                vm.step_argument(frame, (&raw mut class_name).cast())?;
                vm.finish_arguments(frame)?;
            }
            let mut class = unsafe { (*object.cast::<UObject>()).class.cast::<hp2_abi::engine::generated::UClass>() };
            let wanted = vm.host().name_text(class_name)?;
            let mut found = false;
            while !class.is_null() {
                let name = i32::from_ne_bytes(unsafe { (*class.cast::<UObject>()).name });
                if vm.host().name_text(name)? == wanted { found = true; break; }
                class = unsafe { (*class.cast::<hp2_abi::engine::generated::UField>()).super_field.cast() };
            }
            unsafe { result.cast::<u32>().write(u32::from(found)); }
        }
        Op::NewObject => {
            let index = unsafe { (*object.cast::<UObject>()).index };
            let mut outer = if index != -1 { object.cast::<UObject>() } else { std::ptr::null_mut() };
            let mut name = 0_i32;
            let mut flags = 0_u32;
            let mut class = std::ptr::null_mut::<hp2_abi::engine::generated::UClass>();
            unsafe {
                vm.step_argument(frame, (&raw mut outer).cast())?;
                vm.step_argument(frame, (&raw mut name).cast())?;
                vm.step_argument(frame, (&raw mut flags).cast())?;
                vm.step_argument(frame, (&raw mut class).cast())?;
            }
            let allowed = flags & 0x0070_4005;
            let forbidden = flags & !0x0070_4005;
            if forbidden != 0 {
                access.emit_warning(
                    &format!("new: Flags {forbidden:08X} not allowed")
                        .chars().map(u32::from).collect::<Vec<_>>(),
                )?;
            }
            if outer.is_null() { outer = access.transient_package(); }
            if class.is_null() {
                unsafe { result.cast::<*mut UObject>().write(std::ptr::null_mut()); }
            } else {
                let instance = unsafe { access.construct_object(class, outer, name, allowed, std::ptr::null_mut())? };
                unsafe { result.cast::<*mut UObject>().write(instance); }
            }
        }
        Op::Rand => {
            let mut bound = 0_i32;
            unsafe {
                vm.step_argument(frame, (&raw mut bound).cast())?;
                vm.finish_arguments(frame)?;
                if bound > 0 {
                    let raw = access.vm_rand();
                    let index = raw % bound;
                    result.cast::<i32>().write(index);
                    access.observe_vm(crate::core_access::VmObservation::Rand {
                        object: object.cast(), raw, bound, index,
                    })?;
                } else {
                    result.cast::<i32>().write(0);
                }
            }
        }
        Op::VRand => {
            unsafe { vm.finish_arguments(frame)?; }
            loop {
                let unit = |draw: i32| draw as f32 / i32::MAX as f32;
                let vector = hp2_abi::FVector {
                    x: unit(access.vm_rand()).mul_add(2.0, -1.0),
                    y: unit(access.vm_rand()).mul_add(2.0, -1.0),
                    z: unit(access.vm_rand()).mul_add(2.0, -1.0),
                };
                let square = crate::math::dot3(vector, vector);
                if square > 1.0 { continue; }
                let scale = (1.0_f64 / (square as f64).sqrt()) as f32;
                unsafe { result.cast::<hp2_abi::FVector>().write(crate::math::scale(vector, scale)); }
                break;
            }
        }
        Op::RotRand => {
            let mut roll = 0_u32;
            unsafe { vm.step_argument(frame, (&raw mut roll).cast())?; vm.finish_arguments(frame)?; }
            let yaw = access.vm_rand();
            let pitch = access.vm_rand();
            let roll = if roll != 0 { access.vm_rand() } else { 0 };
            let angle = |value: i32| value.wrapping_mul(2) % 65535;
            unsafe {
                result.cast::<crate::math::FRotator>().write(crate::math::FRotator {
                    pitch: angle(pitch), yaw: angle(yaw), roll: angle(roll),
                });
            }
        }
        Op::RandRange => {
            let mut min = 0.0_f32;
            let mut max = 0.0_f32;
            unsafe { vm.step_argument(frame, (&raw mut min).cast())?; vm.step_argument(frame, (&raw mut max).cast())?; vm.finish_arguments(frame)?; }
            let (raw, value) = crate::ansi_trace::with_process_state(|trace| {
                crate::ansi_trace::direct_rand_range_float_with_raw(trace, min, max)
            });
            unsafe { result.cast::<f32>().write(value); }
            access.observe_vm(crate::core_access::VmObservation::RandRange {
                object: object.cast(), raw, min, max, value,
            })?;
        }
        Op::GetEnum => {
            unsafe { result.cast::<i32>().write(0); }
            let mut enumeration = std::ptr::null_mut::<UObject>();
            let mut index = 0_i32;
            unsafe { vm.step_argument(frame, (&raw mut enumeration).cast())?; vm.step_argument(frame, (&raw mut index).cast())?; vm.finish_arguments(frame)?; }
            let enum_class = access.native_class("Enum")?;
            if !enumeration.is_null() && unsafe { crate::object_model::is_a(enumeration.cast(), enum_class.cast()) } && index >= 0 {
                let names = unsafe { &(*enumeration.cast::<hp2_abi::engine::generated::UEnum>()).names };
                let count = unsafe { names.num };
                let data = unsafe { names.data.cast::<i32>() };
                if index < count && !data.is_null() {
                    unsafe { result.cast::<i32>().write(data.add(index as usize).read_unaligned()); }
                }
            }
        }
        Op::NativeTableSlot(index) => {
            return Err(RuntimeError::new("Core VM native belongs to an unavailable engine class/package")
                .with_context(&index.to_string()));
        }
        Op::FRand => unsafe {
            vm.finish_arguments(frame)?;
            result.cast::<f32>().write(access.vm_rand() as f32 / i32::MAX as f32);
        },
        Op::GotoState => {
            let current = access.get_state_name(object.cast())?;
            let mut state_name = current;
            let mut label_name = 0_i32;
            unsafe {
                vm.step_argument(frame, (&raw mut state_name).cast())?;
                vm.step_argument(frame, (&raw mut label_name).cast())?;
                vm.finish_arguments(frame)?;
            }
            if crate::debug_trace::enabled() {
                crate::debug_trace::goto_state(access, object.cast(), current, state_name, label_name);
            }
            access.goto_state_and_label(object.cast(), state_name, label_name, current)?;
        }
        Op::Enable | Op::Disable => {
            let mut probe_name = 0_i32;
            unsafe {
                vm.step_argument(frame, (&raw mut probe_name).cast())?;
                vm.finish_arguments(frame)?;
            }
            let mode = if operation == Op::Enable {
                crate::core_vm_state::ProbeMode::Enable
            } else {
                crate::core_vm_state::ProbeMode::Disable
            };
            access.set_probe(object.cast(), probe_name, mode)?;
        }
        Op::IsInState => {
            let mut state_name = 0_i32;
            unsafe {
                vm.step_argument(frame, (&raw mut state_name).cast())?;
                vm.finish_arguments(frame)?;
            }
            let value = access.is_in_state(object.cast(), state_name)?;
            unsafe { result.cast::<i32>().write(value); }
        }
        Op::GetStateName => {
            unsafe { vm.finish_arguments(frame)?; }
            let value = access.get_state_name(object.cast())?;
            unsafe { result.cast::<i32>().write(value); }
        }
        Op::ResetConfig => {
            unsafe { vm.finish_arguments(frame)?; }
            unsafe { access.reset_config(object.cast())?; }
        }
        Op::Localize => {
            let section = evaluate_string(vm, frame)?;
            let key = evaluate_string(vm, frame)?;
            let package = evaluate_string(vm, frame)?;
            unsafe { vm.finish_arguments(frame)?; }
            let localized = access.localize_text(&section, &key, Some(&package), None, false)?;
            write_string(result.cast(), &localized)?;
        }
        Op::GetLanguage => {
            unsafe { vm.finish_arguments(frame)?; }
            write_string(result.cast(), &access.language_text()?)?;
        }
        Op::DynamicLoadObject => {
            let name_units = evaluate_string(vm, frame)?;
            let mut class = std::ptr::null_mut::<hp2_abi::engine::generated::UClass>();
            let mut may_fail = 0_u32;
            unsafe {
                vm.step_argument(frame, (&raw mut class).cast())?;
                vm.step_argument(frame, (&raw mut may_fail).cast())?;
                vm.finish_arguments(frame)?;
            }
            let loaded = unsafe {
                access.static_load_object(
                    class,
                    std::ptr::null_mut(),
                    &name_units,
                    if may_fail != 0 { 1 } else { 0 },
                )?
            };
            unsafe { result.cast::<*mut UObject>().write(loaded); }
        }
    }
    Ok(StepControl::Continue)
}

fn execute_string_switch<H: VmHost>(
    vm: &VmExecutor<'_, H>,
    frame: *mut hp2_abi::engine::generated::FFrame,
) -> RuntimeResult<()> {
    use hp2_abi::engine::generated::TArrayRaw;
    let mut selector = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    let mut candidate = TArrayRaw { data: std::ptr::null_mut(), num: 0, max: 0 };
    let cursor = unsafe { std::ptr::addr_of_mut!((*frame).code) };
    let outcome = (|| {
        vm.host().string_argument(vm, frame, &mut selector)?;
        loop {
            let token = unsafe { crate::vm_bytecode::read::<u8>(cursor) };
            if token != crate::expression_archive::token::CASE {
                return Err(RuntimeError::new("Expected EX_Case while executing Core switch"));
            }
            let target = unsafe { crate::vm_bytecode::read::<u16>(cursor) };
            if target == u16::MAX { return Ok(()); }
            vm.host().string_argument(vm, frame, &mut candidate)?;
            let mut equal = 0;
            // FString::operator== uses appStricmp, unlike script's == string
            // native. Keep owning strings live for the shared CRT comparison.
            unsafe {
                crate::vm_string::scalar(
                    124, crate::vm_string::view(&mut selector),
                    crate::vm_string::view(&mut candidate), &mut equal,
                );
            }
            if equal != 0 { return Ok(()); }
            let base = unsafe {
                (*(*frame).node.cast::<hp2_abi::engine::generated::UStruct>()).script.data.cast::<u8>()
            };
            if base.is_null() { return Err(RuntimeError::new("Core switch has no script base")); }
            unsafe { cursor.write(base.add(target as usize)); }
        }
    })();
    let destroy_candidate = vm.host().release_string(&mut candidate);
    let destroy_selector = vm.host().release_string(&mut selector);
    outcome.and(destroy_candidate).and(destroy_selector)
}

fn execute_flow<H: VmHost>(
    access: CoreAccess<'_>,
    vm: &VmExecutor<'_, H>,
    operation: crate::vm_owner::VmFlowOp,
    object: *mut c_void,
    frame: *mut hp2_abi::engine::generated::FFrame,
    _result: *mut c_void,
) -> RuntimeResult<crate::vm_owner::StepControl> {
    use crate::vm_owner::{StepControl, VmFlowOp as Op};
    let cursor = unsafe { std::ptr::addr_of_mut!((*frame).code) };
    match operation {
        Op::Jump => {
            let target = unsafe { crate::vm_bytecode::read::<u16>(cursor) };
            let base = unsafe { (*(*frame).node.cast::<hp2_abi::engine::generated::UStruct>()).script.data.cast::<u8>() };
            if base.is_null() { return Err(RuntimeError::new("Core VM jump has no script base")); }
            unsafe { cursor.write(base.add(target as usize)); }
        }
        Op::JumpIfNot => {
            let target = unsafe { crate::vm_bytecode::read::<u16>(cursor) };
            let mut condition = 0_u32;
            unsafe { vm.step_argument(frame, (&raw mut condition).cast())?; }
            if condition == 0 {
                let base = unsafe { (*(*frame).node.cast::<hp2_abi::engine::generated::UStruct>()).script.data.cast::<u8>() };
                if base.is_null() { return Err(RuntimeError::new("Core VM branch has no script base")); }
                unsafe { cursor.write(base.add(target as usize)); }
            }
        }
        Op::Assert => {
            let line = unsafe { crate::vm_bytecode::read::<u16>(cursor) };
            let mut condition = 0_u32;
            unsafe { vm.step_argument(frame, (&raw mut condition).cast())?; }
            if condition == 0 {
                return Err(RuntimeError::new("Script assertion failed").with_context(&line.to_string()));
            }
        }
        Op::Case => {
            let target = unsafe { crate::vm_bytecode::read::<u16>(cursor) };
            if target != u16::MAX {
                let mut scratch = hp2_abi::engine::generated::TArrayRaw {
                    data: std::ptr::null_mut(), num: 0, max: 0,
                };
                unsafe { vm.step_argument(frame, (&raw mut scratch).cast())?; }
                // Legacy Case intentionally retains this temporary string until the frame exits.
            }
        }
        Op::GotoLabel => {
            let mut name = 0_i32;
            unsafe { vm.step_argument(frame, (&raw mut name).cast())?; }
            access.goto_label_or_warn(object.cast(), name)?;
        }
        Op::Switch => {
            let width = unsafe { crate::vm_bytecode::read::<u8>(cursor) } as usize;
            if width == 0 {
                execute_string_switch(vm, frame)?;
            } else {
                let mut selector = [0_u8; 256];
                unsafe { vm.step_argument(frame, selector.as_mut_ptr().cast())?; }
                loop {
                    let token = unsafe { crate::vm_bytecode::read::<u8>(cursor) };
                    if token != crate::expression_archive::token::CASE {
                        return Err(RuntimeError::new("Expected EX_Case while executing Core switch"));
                    }
                    let target = unsafe { crate::vm_bytecode::read::<u16>(cursor) };
                    if target == u16::MAX { break; }
                    let mut candidate = [0_u8; 256];
                    unsafe { vm.step_argument(frame, candidate.as_mut_ptr().cast())?; }
                    if selector[..width] == candidate[..width] { break; }
                    let base = unsafe { (*(*frame).node.cast::<hp2_abi::engine::generated::UStruct>()).script.data.cast::<u8>() };
                    if base.is_null() { return Err(RuntimeError::new("Core switch has no script base")); }
                    unsafe { cursor.write(base.add(target as usize)); }
                }
            }
        }
        Op::DebugInfo => {
            let marker = unsafe { crate::vm_bytecode::read::<i32>(cursor) };
            if marker != 100 {
                unsafe { crate::vm_bytecode::advance(cursor, -5); }
            } else {
                let line = unsafe { crate::vm_bytecode::read::<i32>(cursor) };
                let input = unsafe { crate::vm_bytecode::read::<i32>(cursor) };
                let mut text = Vec::new();
                loop {
                    let byte = unsafe { crate::vm_bytecode::read::<u8>(cursor) };
                    if byte == 0 { break; }
                    text.push(u32::from(byte));
                }
                // SAFETY: the VM executor invokes this synchronously with the live frame/object.
                unsafe { access.debug_info(object.cast(), frame, &text, line, input)?; }
            }
        }
    }
    let _ = object;
    Ok(StepControl::Continue)
}

fn event_boundary(
    access: CoreAccess<'_>,
    phase: crate::vm_owner::EventBoundary,
    _object: *mut c_void,
    _function: *mut UFunction,
    _frame: *mut hp2_abi::engine::generated::FFrame,
) -> RuntimeResult<()> {
    match phase {
        crate::vm_owner::EventBoundary::Enter => {
            crate::timing_globals::enter_script_scope();
            if access.vm_enter_event() {
                let category = access.intern_str("Critical", crate::name_owner::FindName::Add)?;
                let message = format!(
                    "Infinite script recursion ({} calls) detected",
                    crate::vm_guard::HP2_VM_RECURSE_LIMIT,
                ).chars().map(u32::from).collect::<Vec<_>>();
                access.emit_log(category, &message)?;
            }
            Ok(())
        }
        crate::vm_owner::EventBoundary::Exit => {
            access.vm_exit_event();
            crate::timing_globals::leave_script_scope();
            Ok(())
        }
    }
}

fn check_runaway(
    access: CoreAccess<'_>,
    _object: *mut c_void,
    _function: *mut UFunction,
    _frame: *mut hp2_abi::engine::generated::FFrame,
    _steps: u64,
) -> RuntimeResult<()> {
    if !access.vm_runaway_step() {
        return Ok(());
    }
    let suppressed = command_line_has_param(access.command_line(), b"norunaway");
    if !suppressed {
        let category = access.intern_str("Critical", crate::name_owner::FindName::Add)?;
        let message = format!(
            "Runaway loop detected (over {} iterations)",
            crate::vm_guard::HP2_VM_RUNAWAY_LIMIT,
        ).chars().map(u32::from).collect::<Vec<_>>();
        access.emit_log(category, &message)?;
    }
    access.vm_reset_runaway();
    Ok(())
}

fn command_line_has_param(command_line: &[u32], needle: &[u8]) -> bool {
    command_line.split(|unit| *unit == u32::from(b' ') || *unit == u32::from(b'\t'))
        .any(|token| {
            token.len() == needle.len()
                && token.iter().zip(needle).all(|(&unit, &byte)| {
                    (unit as u8).to_ascii_lowercase() == byte
                })
        })
}
