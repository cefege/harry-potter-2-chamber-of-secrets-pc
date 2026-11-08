//! Canonical UEngine identity and lifetime policy.

use hp2_abi::engine::generated::{UObject, UEngine};
pub use crate::core_owner::{ActorLifecycleEvent, ActorLifecycleObserver, AudioNoteDestroy, AudioUnregisterSound};
use hp2_core::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_class_specs::{NativeClassSpec, NativeRecordKind, StartupProperty, StartupPropertyKind},
    object_archive::ObjectArchive,
    object_owner::ClassOps,
};
use std::{alloc::Layout, ptr, ptr::NonNull};

const OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const CLASS_CONFIG: u32 = 0x0000_0004;
const CLASS_TRANSIENT: u32 = 0x0000_0008;
const CLASS_ABSTRACT: u32 = 0x0000_0001;
const BASE: ClassOps = NativeRecordKind::Subsystem.operations();

const CONFIG_PROPERTIES: &[StartupProperty] = &[
    StartupProperty {
        name: "CacheSizeMegs",
        kind: StartupPropertyKind::Int,
        category: "Settings",
        property_flags: 0x0000_4000,
        object_flags: 4,
        inner_name: None,
        offset: std::mem::offset_of!(UEngine, cache_size_megs),
        array_dim: 1,
    },
    StartupProperty {
        name: "UseSound",
        kind: StartupPropertyKind::Bool { bit_mask: 1 },
        category: "Settings",
        property_flags: 0x0000_4000,
        object_flags: 4,
        inner_name: None,
        offset: std::mem::offset_of!(UEngine, use_sound),
        array_dim: 1,
    },
];
/// Canonical UEngine class declaration; superclass identity is Core.Subsystem.
pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "Engine",
    package: "Engine",
    super_name: Some("Subsystem"),
    within: "Object",
    flags: CLASS_CONFIG | CLASS_TRANSIENT | CLASS_ABSTRACT,
    object_flags: OBJECT_FLAGS,
    config: "System",
    guid: [0; 4],
    kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<UEngine>()),
    operations: Some(OPERATIONS),
    properties: CONFIG_PROPERTIES,
    defaults: &[],
}];

pub(crate) const OPERATIONS: ClassOps = {
    let mut operations = BASE;
    operations.destroy = destroy;
    operations.serialize = serialize;
    operations.visit_references = serialize;
    operations
};


unsafe fn destroy(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical UEngine")); }
    access.remove_root(object);
    let engine = object.cast::<UEngine>();
    unsafe {
        (*engine).audio = ptr::null_mut();
        (*engine).render = ptr::null_mut();
        (*engine).client = ptr::null_mut();
    }
    let state = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("UEngine destruction has no canonical Engine owner"))?;
    crate::url_owner::UrlOwner::static_exit(access)?;
    state.memory.exit()?;
    let mut cache = state.cache.try_borrow_mut()
        .map_err(|_| RuntimeError::new("Engine cache teardown reentered"))?
        .take().ok_or_else(|| RuntimeError::new("Engine cache is unavailable during teardown"))?;
    let result = cache.exit_in_place(access);
    *state.cache.borrow_mut() = Some(cache);
    result?;
    // SAFETY: UEngine::Destroy ends with qualified USubsystem::Destroy.
    unsafe { (BASE.destroy)(access, object) }
}

unsafe fn serialize(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    // SAFETY: Subsystem is the actual superclass and owns the UObject archive prefix.
    unsafe { (BASE.serialize)(access, object, archive)?; }
    let engine = object.cast::<UEngine>();
    for slot in unsafe { [
        ptr::addr_of_mut!((*engine).cylinder),
        ptr::addr_of_mut!((*engine).o_cylinder),
        ptr::addr_of_mut!((*engine).oval_cylinder),
        ptr::addr_of_mut!((*engine).o_oval_cylinder),
        ptr::addr_of_mut!((*engine).r#box),
        ptr::addr_of_mut!((*engine).client),
        ptr::addr_of_mut!((*engine).render),
        ptr::addr_of_mut!((*engine).audio),
    ] } {
        // SAFETY: each field is a writable object-reference slot in the live probed UEngine.
        unsafe { archive.serialize_object_slot(slot.cast::<*mut UObject>())?; }
    }
    Ok(())
}

/// Initializes owner-local Engine policy and roots the canonical engine identity.
///
/// # Safety
/// `engine` must be a live canonical UEngine allocated by this CoreAccess owner.
pub unsafe fn init(access: CoreAccess<'_>, engine: NonNull<UEngine>) -> RuntimeResult<()> {
    let state = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("UEngine initialization has no canonical Engine owner"))?;
    crate::url_owner::UrlOwner::static_init(access)?;
    let megs = if access.process_mode().is_client { unsafe { engine.as_ref().cache_size_megs } } else { crate::uengine::CACHE_MEGS_MIN };
    let bytes = megs.clamp(crate::uengine::CACHE_MEGS_MIN, crate::uengine::CACHE_MEGS_MAX)
        .checked_mul(1024 * 1024).ok_or_else(|| RuntimeError::new("Engine cache size overflow"))?;
    let mut cache = state.cache.try_borrow_mut()
        .map_err(|_| RuntimeError::new("Engine cache initialization reentered"))?
        .take().ok_or_else(|| RuntimeError::new("Engine cache is already initialized"))?;
    let result = cache.initialize_in_place(access, bytes, crate::uengine::CACHE_ALIGN, 0);
    *state.cache.borrow_mut() = Some(cache);
    result?;
    let general = [71, 101, 110, 101, 114, 97, 108, 0];
    let core = [67, 111, 114, 101, 0];
    let yes = access.localize_text(&general, &[89, 101, 115, 0], Some(&core), None, false)?;
    let no = access.localize_text(&general, &[78, 111, 0], Some(&core), None, false)?;
    let initial = |text: &[u32]| text.first()
        .map(|&unit| hp2_core::name::to_upper(unit as hp2_core::ansi::Tchar) as u32)
        .unwrap_or(0);
    crate::uengine::publish_yes_no_keys(initial(&yes), initial(&no));

    let engine_ptr = engine.as_ptr();
    for (class_name, field) in [
        ("Engine.Primitive", 0),
        ("Engine.OrientedCylinder", 1),
        ("Engine.AlignedOvalCylinder", 2),
        ("Engine.OrientedOvalCylinder", 3),
        ("Engine.Box", 4),
    ] {
        let class = access.native_class(class_name)?;
        // Native `operator new` uses the transient package, NAME_None and SetFlags=0.
        // SAFETY: the class and outer are identities in this canonical Core graph.
        let object = unsafe {
            access.construct_object(class, access.transient_package(), 0, 0, ptr::null_mut())?
        };
        unsafe {
            match field {
                0 => (*engine_ptr).cylinder = object.cast(),
                1 => (*engine_ptr).o_cylinder = object.cast(),
                2 => (*engine_ptr).oval_cylinder = object.cast(),
                3 => (*engine_ptr).o_oval_cylinder = object.cast(),
                _ => (*engine_ptr).r#box = object.cast(),
            }
        }
    }
    // SAFETY: the engine pointer is live in this owner's canonical object graph.
    unsafe { access.add_root(engine_ptr.cast()); }
    let category = access.intern_str("Init", hp2_core::name_owner::FindName::Add)?;
    access.emit_log(category, &[85, 110, 114, 101, 97, 108, 32, 101, 110, 103, 105, 110, 101, 32, 105, 110, 105, 116, 105, 97, 108, 105, 122, 101, 100])?;
    Ok(())
}
/// Runs the canonical `UEngine::Exit` audio teardown through the shared source policy.
///
/// # Safety
/// `engine` is a live canonical UEngine owned by `access`.
pub unsafe fn exit(access: CoreAccess<'_>, engine: *mut UEngine) -> RuntimeResult<()> {
    struct CoreExitAudio<'a>(CoreAccess<'a>);
    impl crate::uengine::ExitAudioOps for CoreExitAudio<'_> {
        type Error = RuntimeError;
        fn audio(&mut self, engine: *mut std::ffi::c_void) -> RuntimeResult<*mut std::ffi::c_void> {
            if engine.is_null() { return Err(RuntimeError::new("Null canonical UEngine")); }
            Ok(unsafe { (*engine.cast::<UEngine>()).audio.cast() })
        }
        fn delete_audio(&mut self, audio: *mut std::ffi::c_void) -> RuntimeResult<()> {
            if audio.is_null() { return Ok(()); }
            unsafe { self.0.delete_object(audio.cast()) }
        }
        fn set_audio(&mut self, engine: *mut std::ffi::c_void, audio: *mut std::ffi::c_void) -> RuntimeResult<()> {
            if engine.is_null() { return Err(RuntimeError::new("Null canonical UEngine")); }
            unsafe { (*engine.cast::<UEngine>()).audio = audio; }
            Ok(())
        }
    }
    unsafe { crate::uengine::exit_audio_with_ops(&mut CoreExitAudio(access), engine.cast()) }
}


pub(crate) fn platform_state(
    access: CoreAccess<'_>,
) -> RuntimeResult<std::rc::Rc<crate::platform_owner::PlatformOwnerState>> {
    access.owner_context::<crate::core_owner::EngineState>()
        .map(|state| std::rc::Rc::clone(&state.platform))
        .ok_or_else(|| RuntimeError::new("Canonical Engine platform owner is unavailable"))
}
/// Runs source `UEngine::Flush` through the shared Native/Core policy.
///
/// # Safety
/// `engine` must be a live canonical UEngine for this owner.
pub(crate) unsafe fn flush(
    access: CoreAccess<'_>,
    engine: *mut UEngine,
    allow_precache: bool,
) -> RuntimeResult<()> {
    if engine.is_null() { return Err(RuntimeError::new("Null canonical UEngine Flush")); }
    let state = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("UEngine Flush has no canonical Engine owner"))?;
    unsafe {
        crate::uengine::flush_with(
            engine.cast(),
            i32::from(allow_precache),
            |_| {
                let mut cache = state.cache.try_borrow_mut()
                    .map_err(|_| RuntimeError::new("Engine cache flush reentered"))?;
                cache.as_mut().ok_or_else(|| RuntimeError::new("Engine cache is unavailable during Flush"))?
                    .flush(access, 0, u32::MAX, 0)
            },
            |engine| Ok(unsafe { (*engine.cast::<UEngine>()).client }),
            |client, allow| unsafe {
                crate::platform_owner::flush(access, client.cast(), allow != 0)
            },
        )
    }
}

