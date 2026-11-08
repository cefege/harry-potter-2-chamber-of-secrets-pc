//! Canonical moving-brush ownership and the temporary Core-backed BSP host.

use crate::{actor, brush_owner, dynbsp::{DynBsp, DynBspHost, LogKind, Obj, Pool}, model::{self, ModelArray}, model_archive::Hp2ModelArchiveField};
use hp2_abi::{FCoords, FVector};
use hp2_abi::engine::generated::{AActor, ABrush, AMover, FPoly, FRotator, ULevel, UModel, UObject};
use hp2_abi::render_scene::{FBspNode, FBspSurf, FVert};
use hp2_core::{ansi, array_storage, core_access::{CoreAccess, RuntimeError, RuntimeResult}, object_archive::ObjectArchive, object_model};
use std::{ffi::{CStr, c_void}, panic::{AssertUnwindSafe, catch_unwind, resume_unwind}, ptr::{self, NonNull}, rc::Rc};

#[derive(Clone, Copy)]
struct Classes { brush: *const c_void, mover: *const c_void }

struct TrackerOwner {
    state: NonNull<DynBsp>,
    level: *mut ULevel,
    model: *mut UModel,
    classes: Classes,
}

impl TrackerOwner {
    fn host<'a>(&self, access: CoreAccess<'a>) -> CoreHost<'a> {
        CoreHost { access, level: self.level, model: self.model, classes: self.classes }
    }
}

impl Drop for TrackerOwner {
    fn drop(&mut self) {
        // SAFETY: ownership came from Box::into_raw; model restoration precedes release.
        unsafe { drop(Box::from_raw(self.state.as_ptr())); }
    }
}

fn boundary<T>(operation: impl FnOnce() -> T) -> RuntimeResult<T> {
    catch_unwind(AssertUnwindSafe(operation)).map_err(|failure| {
        let failure = match failure.downcast::<RuntimeError>() {
            Ok(error) => return *error,
            Err(failure) => failure,
        };
        let failure = match failure.downcast::<String>() {
            Ok(message) => return RuntimeError::new(&message).with_context("canonical moving-brush tracker"),
            Err(failure) => failure,
        };
        match failure.downcast::<&'static str>() {
            Ok(message) => RuntimeError::new(&message).with_context("canonical moving-brush tracker"),
            Err(failure) => resume_unwind(failure),
        }
    })
}

fn demand<T>(result: RuntimeResult<T>) -> T {
    match result { Ok(value) => value, Err(error) => resume_unwind(Box::new(error)) }
}

fn array_result<T>(result: Result<T, i32>, operation: &str) -> T {
    demand(result.map_err(|code| RuntimeError::new("Canonical moving-brush array operation failed")
        .with_context(&format!("{operation} (status {code})"))))
}

unsafe fn owner(level: *mut ULevel) -> RuntimeResult<Rc<TrackerOwner>> {
    if level.is_null() { return Err(RuntimeError::new("Null Level in moving-brush operation")); }
    let pointer = unsafe { (*level).brush_tracker.cast::<TrackerOwner>() };
    if pointer.is_null() { return Err(RuntimeError::new("Level has no canonical moving-brush tracker")); }
    // SAFETY: the private Level slot owns one Rc; retain it before any host callback.
    unsafe { Rc::increment_strong_count(pointer); Ok(Rc::from_raw(pointer)) }
}

/// Constructs the real moving-brush pools over a canonical Level and its model.
///
/// # Safety
/// Level, its model, actors and their brush models are live canonical identities;
/// the tracker slot is empty, and all models outlive tracker destruction.
pub unsafe fn initialize(access: CoreAccess<'_>, level: *mut ULevel) -> RuntimeResult<()> {
    if level.is_null() { return Err(RuntimeError::new("Null Level in moving-brush initialization")); }
    if unsafe { !(*level).brush_tracker.is_null() } {
        return Err(RuntimeError::new("Level moving-brush tracker is already initialized"));
    }
    let model = unsafe { (*level).model.cast::<UModel>() };
    if model.is_null() { return Err(RuntimeError::new("Moving-brush Level has no model")); }
    let classes = Classes {
        brush: access.native_class("Engine.Brush")?.cast(),
        mover: access.native_class("Engine.Mover")?.cast(),
    };
    let mut host = CoreHost { access, level, model, classes };
    let state = boundary(|| unsafe { DynBsp::new(&mut host) })?;
    // SAFETY: a Box is non-null and transfers sole allocation ownership to this owner.
    let state = unsafe { NonNull::new_unchecked(Box::into_raw(state)) };
    let tracker = Rc::new(TrackerOwner { state, level, model, classes });
    // This private Rust handle is never a native FMovingBrushTrackerBase vptr.
    unsafe { (*level).brush_tracker = Rc::into_raw(tracker).cast_mut().cast(); }
    Ok(())
}

/// Restores the original model arrays, then releases the Level's tracker ownership.
///
/// # Safety
/// Level's slot is null or is the private handle created by initialize; its model
/// remains live until restoration finishes. Active operations retain their own Rc.
pub unsafe fn destroy(access: CoreAccess<'_>, level: *mut ULevel) -> RuntimeResult<()> {
    if level.is_null() { return Err(RuntimeError::new("Null Level in moving-brush destruction")); }
    let slot = unsafe { ptr::addr_of_mut!((*level).brush_tracker) };
    let pointer = unsafe { slot.read_unaligned().cast::<TrackerOwner>() };
    if pointer.is_null() { return Ok(()); }
    unsafe { slot.write_unaligned(ptr::null_mut()); }
    // SAFETY: consume exactly the Level slot's owning reference after detaching it.
    let tracker = unsafe { Rc::from_raw(pointer) };
    let mut host = tracker.host(access);
    boundary(|| unsafe { DynBsp::shutdown(tracker.state.as_ptr(), &mut host) })
}

/// Runs the source movement wrapper; Entry and ordinary non-brush actors
/// intentionally require no moving-brush tracker update.
///
/// # Safety
/// Level and any non-null actor are live canonical records.
pub(crate) unsafe fn update_moving_brush_tracker(
    access: CoreAccess<'_>, level: *mut ULevel, actor: *mut AActor,
) -> RuntimeResult<()> {
    if level.is_null() { return Err(RuntimeError::new("Null Level in moving-brush movement update")); }
    unsafe {
        crate::dynbsp::update_moving_brush_tracker_with(
            (*level).brush_tracker, actor.cast(),
            |actor| {
                let brush = access.native_class("Engine.Brush")?;
                actor::is_moving_brush_with(actor.cast(), |object, class| {
                    debug_assert_eq!(class, actor::class::ABRUSH);
                    Ok(object_model::is_a(object, brush.cast()))
                }).map_err(|status| RuntimeError::new(&format!("Moving-brush class query failed ({status})")))
            },
            |_tracker, actor| update(access, level, actor.cast()),
        )
    }
}

/// Updates a moving brush; a null actor requests the native all-brush update.
///
/// # Safety
/// Level owns an initialized tracker; every reached actor/model remains live.
pub unsafe fn update(access: CoreAccess<'_>, level: *mut ULevel, actor: *mut AActor) -> RuntimeResult<()> {
    let tracker = unsafe { owner(level)? };
    let mut host = tracker.host(access);
    boundary(|| unsafe { DynBsp::update(tracker.state.as_ptr(), &mut host, actor.cast()) })
}

/// Removes a brush's dynamic nodes through the shared native BSP policy.
///
/// # Safety
/// Level owns an initialized tracker; actor and its brush model remain live.
pub unsafe fn flush(access: CoreAccess<'_>, level: *mut ULevel, actor: *mut AActor) -> RuntimeResult<()> {
    let tracker = unsafe { owner(level)? };
    let mut host = tracker.host(access);
    boundary(|| unsafe { DynBsp::flush(tracker.state.as_ptr(), &mut host, actor.cast()) })
}

/// Accounts the four actual tracker bookkeeping pools in a canonical archive.
///
/// # Safety
/// Level owns an initialized tracker; it is accessed only on its engine thread.
pub unsafe fn count_bytes(level: *mut ULevel, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    let tracker = unsafe { owner(level)? };
    let counts = unsafe { tracker.state.as_ref().count_bytes() };
    for (count, capacity) in counts { archive.count_bytes(count, capacity)?; }
    Ok(())
}

/// Classifies a surface using the tracker's actual pre-expansion surface count.
///
/// # Safety
/// Level owns an initialized tracker; it is accessed only on its engine thread.
pub unsafe fn surf_is_dynamic(level: *mut ULevel, surface: i32) -> RuntimeResult<bool> {
    let tracker = unsafe { owner(level)? };
    Ok(unsafe { tracker.state.as_ref().surf_is_dynamic(surface) })
}

struct CoreHost<'a> {
    access: CoreAccess<'a>,
    level: *mut ULevel,
    model: *mut UModel,
    classes: Classes,
}

impl CoreHost<'_> {
    fn array(&self, model: *mut UModel, field: Hp2ModelArchiveField) -> ModelArray {
        let array = array_result(unsafe { model::model_array(model, field) }, "model array lookup");
        let count = unsafe { array.slots.num.read() };
        let capacity = unsafe { array.slots.max.read() };
        let data = unsafe { array.slots.data.read_unaligned() };
        if count < 0 || capacity < count || (count > 0 && data.is_null()) {
            demand::<()>(Err(RuntimeError::new("Invalid canonical moving-brush model storage")));
        }
        array
    }

    fn pool(&self, pool: Pool) -> ModelArray {
        let field = match pool {
            Pool::Surfs => Hp2ModelArchiveField::Surfs,
            Pool::Vectors => Hp2ModelArchiveField::Vectors,
            Pool::Points => Hp2ModelArchiveField::Points,
            Pool::Nodes => Hp2ModelArchiveField::Nodes,
            Pool::Verts => Hp2ModelArchiveField::Verts,
        };
        self.array(self.model, field)
    }

    fn elements<T>(&mut self, array: ModelArray) -> &mut [T] {
        if array.width as usize != std::mem::size_of::<T>() {
            demand::<()>(Err(RuntimeError::new("Moving-brush model element layout mismatch")));
        }
        let count = unsafe { array.slots.num.read() } as usize;
        if count == 0 { return &mut []; }
        // SAFETY: array() validated storage; caller selects its matching compiler-probed T.
        unsafe { std::slice::from_raw_parts_mut(array.slots.data.read_unaligned().cast(), count) }
    }

    fn ansi_name(&self, mut text: Vec<u32>) -> String {
        text.push(0);
        // Native host names pass through appToAnsi, then CHost's CStr conversion.
        let bytes = unsafe { ansi::to_ansi_string(text.as_ptr().cast()) };
        unsafe { CStr::from_ptr(bytes.cast()) }.to_string_lossy().into_owned()
    }
}

impl DynBspHost for CoreHost<'_> {
    fn num(&self, pool: Pool) -> i32 { unsafe { self.pool(pool).slots.num.read() } }

    fn set_num(&mut self, pool: Pool, count: i32) {
        if count < 0 { demand::<()>(Err(RuntimeError::new("Negative moving-brush array count"))); }
        let array = self.pool(pool);
        let old = unsafe { array.slots.num.read() };
        array_result(unsafe { array_storage::set(array.slots, count, array.width, ptr::null(), array_storage::system_allocator()) }, "model array Set");
        if count > old {
            // Native TArray::Set does not run element destructors; only growth is zeroed.
            unsafe {
                let data = array.slots.data.read_unaligned().cast::<u8>();
                ptr::write_bytes(data.add(old as usize * array.width as usize), 0,
                    (count - old) as usize * array.width as usize);
            }
        }
    }

    fn shrink(&mut self, pool: Pool) {
        let array = self.pool(pool);
        array_result(unsafe { array_storage::shrink(array.slots, array.width, ptr::null(), array_storage::system_allocator()) }, "model array Shrink");
    }

    fn remove(&mut self, pool: Pool, index: i32, count: i32) {
        let array = self.pool(pool);
        let num = unsafe { array.slots.num.read() };
        if index < 0 || count < 0 || index.checked_add(count).is_none_or(|end| end > num) {
            demand::<()>(Err(RuntimeError::new("Invalid moving-brush model removal range")));
        }
        let allocator = array_storage::system_allocator();
        if matches!(array.kind, model::ModelArrayKind::FBspSurf) {
            let data = unsafe { array.slots.data.read_unaligned().cast::<u8>() };
            for element in index..index + count {
                let pointer = unsafe { data.add(element as usize * array.width as usize) };
                array_result(unsafe { model::destroy_model_element(array.kind, pointer.cast(), allocator) }, "model element destruction");
            }
        }
        array_result(unsafe { array_storage::remove(array.slots, index, count, array.width, ptr::null(), allocator) }, "model array Remove");
    }

    fn surfs_mut(&mut self) -> &mut [FBspSurf] { self.elements(self.pool(Pool::Surfs)) }
    fn vectors_mut(&mut self) -> &mut [FVector] { self.elements(self.pool(Pool::Vectors)) }
    fn points_mut(&mut self) -> &mut [FVector] { self.elements(self.pool(Pool::Points)) }
    fn verts_mut(&mut self) -> &mut [FVert] { self.elements(self.pool(Pool::Verts)) }
    fn nodes_mut(&mut self) -> &mut [FBspNode] { self.elements(self.pool(Pool::Nodes)) }

    fn level_actor_count(&self) -> i32 {
        demand(unsafe { crate::level_owner::actor_count(self.level) })
    }

    fn level_actor(&self, index: i32) -> Obj {
        if index < 0 || index >= self.level_actor_count() {
            demand::<()>(Err(RuntimeError::new("Moving-brush actor index is out of range")));
        }
        unsafe { (*self.level).base.actors.data.cast::<Obj>().add(index as usize).read_unaligned() }
    }

    fn level_full_name(&self) -> String {
        self.ansi_name(demand(unsafe { self.access.object_full_name(self.level.cast::<UObject>()) }))
    }

    fn is_moving_brush(&self, value: Obj) -> bool {
        array_result(unsafe {
            actor::is_moving_brush_with(value.cast(), |object, class| {
                debug_assert_eq!(class, actor::class::ABRUSH);
                Ok(object_model::is_a(object, self.classes.brush))
            })
        }, "moving-brush class predicate")
    }

    fn is_mover(&self, value: Obj) -> bool { unsafe { object_model::is_a(value, self.classes.mover) } }
    fn actor_name(&self, value: Obj) -> String {
        let index = i32::from_ne_bytes(unsafe { (*value.cast::<UObject>()).name });
        self.ansi_name(demand(self.access.name_text(index)))
    }
    fn location(&self, value: Obj) -> FVector { unsafe { (*value.cast::<AActor>()).location } }
    fn rotation(&self, value: Obj) -> FRotator { unsafe { (*value.cast::<AActor>()).rotation } }
    fn pre_pivot(&self, value: Obj) -> FVector { unsafe { (*value.cast::<AActor>()).pre_pivot } }
    fn saved_pos(&self, value: Obj) -> FVector { unsafe { (*value.cast::<AMover>()).saved_pos } }
    fn set_saved_pos(&mut self, value: Obj, position: FVector) { unsafe { (*value.cast::<AMover>()).saved_pos = position; } }
    fn saved_rot(&self, value: Obj) -> FRotator { unsafe { (*value.cast::<AMover>()).saved_rot } }
    fn set_saved_rot(&mut self, value: Obj, rotation: FRotator) { unsafe { (*value.cast::<AMover>()).saved_rot = rotation; } }
    fn assimilated(&self, value: Obj) -> bool { unsafe { (*value.cast::<AActor>()).b_assimilated() } }
    fn set_assimilated(&mut self, value: Obj, assimilated: bool) { unsafe { (*value.cast::<AActor>()).set_b_assimilated(assimilated); } }
    fn special_lit(&self, value: Obj) -> bool { unsafe { (*value.cast::<AActor>()).b_special_lit() } }
    fn brush(&self, value: Obj) -> Obj { unsafe { (*value.cast::<AActor>()).brush } }
    fn build_coords(&mut self, value: Obj) -> (FCoords, FCoords, f32) { unsafe { brush_owner::build_coords(value.cast::<ABrush>()) } }
    fn brush_mover_link(&self, brush: Obj) -> i32 { unsafe { (*brush.cast::<UModel>()).mover_link } }
    fn set_brush_mover_link(&mut self, brush: Obj, link: i32) { unsafe { (*brush.cast::<UModel>()).mover_link = link; } }
    fn brush_polys(&mut self, brush: Obj) -> &mut [FPoly] {
        let array = self.array(brush.cast(), Hp2ModelArchiveField::Polys);
        self.elements(array)
    }

    fn debug(&mut self, kind: LogKind, message: &str) {
        let category = match kind { LogKind::Init => "Init", LogKind::Log => "Log", LogKind::Warning => "Warning" };
        let category = demand(self.access.intern_str(category, hp2_core::name_owner::FindName::Find));
        let text: Vec<u32> = ansi::ansi_scalars(message.bytes()).collect();
        demand(self.access.emit_log(category, &text));
    }
}
