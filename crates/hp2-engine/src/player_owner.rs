//! Canonical Rust metadata and lifetime for Engine.UPlayer.

use hp2_abi::engine::generated::{APlayerPawn, UPlayer, UObject};
use hp2_core::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_class_specs::{NativeClassSpec, NativeRecordKind, StartupProperty, StartupPropertyKind},
    object_archive::ObjectArchive,
    object_owner::ClassOps,
};
use std::{alloc::Layout, ptr};

const BASE: ClassOps = NativeRecordKind::Object.operations();
const OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const CLASS_ABSTRACT: u32 = 1;
const CLASS_CONFIG: u32 = 4;
const CLASS_TRANSIENT: u32 = 8;
const CPF_TRANSIENT: u32 = 0x0000_2000;

const PROPERTIES: &[StartupProperty] = &[StartupProperty {
    name: "Actor",
    kind: StartupPropertyKind::Object { class: "Engine.PlayerPawn" },
    category: "Player",
    property_flags: CPF_TRANSIENT,
    object_flags: 4,
    inner_name: None,
    offset: std::mem::offset_of!(UPlayer, actor),
    array_dim: 1,
}];

/// Native `Engine.UPlayer` declaration, registered before network connections and viewports.
pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "Player", package: "Engine", super_name: Some("Object"), within: "Object",
    flags: CLASS_ABSTRACT | CLASS_CONFIG | CLASS_TRANSIENT, object_flags: OBJECT_FLAGS,
    config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<UPlayer>()), operations: Some(OPERATIONS),
    properties: PROPERTIES, defaults: &[],
}];

pub(crate) const OPERATIONS: ClassOps = {
    let mut operations = BASE;
    operations.construct = construct;
    operations.destroy = destroy;
    operations.serialize = serialize;
    operations.visit_references = visit_references;
    operations
};

unsafe fn construct(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical UPlayer")); }
    // UPlayer's native default constructor has an empty body; Core already
    // installed the class defaults, including any configured Actor reference.
    unsafe { (BASE.construct)(access, object) }
}

unsafe fn destroy(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical UPlayer")); }
    let player = object.cast::<UPlayer>();
    unsafe { crate::player::destroy_with_ops(&mut PlayerDestroyOps(access), player.cast())?; }
    unsafe { (BASE.destroy)(access, object) }
}

struct PlayerDestroyOps<'a>(CoreAccess<'a>);
impl crate::player::PlayerDestroyOps for PlayerDestroyOps<'_> {
    type Error = RuntimeError;
    fn is_running(&mut self) -> RuntimeResult<bool> {
        Ok(self.0.is_running())
    }
    unsafe fn actor(&mut self, player: *mut std::ffi::c_void) -> RuntimeResult<*mut std::ffi::c_void> {
        Ok(unsafe { (*player.cast::<UPlayer>()).actor })
    }
    unsafe fn level(&mut self, actor: *mut std::ffi::c_void) -> RuntimeResult<*mut std::ffi::c_void> {
        Ok(unsafe { (*actor.cast::<APlayerPawn>()).base.base.x_level })
    }
    unsafe fn clear_player(&mut self, actor: *mut std::ffi::c_void) -> RuntimeResult<()> {
        unsafe { (*actor.cast::<APlayerPawn>()).player = ptr::null_mut(); }
        Ok(())
    }
    unsafe fn destroy_actor(&mut self, level: *mut std::ffi::c_void, actor: *mut std::ffi::c_void) -> RuntimeResult<()> {
        unsafe { crate::level_lifecycle_owner::destroy_actor(self.0, level.cast(), actor.cast(), true)?; }
        Ok(())
    }
    unsafe fn clear_actor(&mut self, player: *mut std::ffi::c_void) -> RuntimeResult<()> {
        unsafe { (*player.cast::<UPlayer>()).actor = ptr::null_mut(); }
        Ok(())
    }
}

unsafe fn serialize(
    access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe { (BASE.serialize)(access, object, archive) }
}

unsafe fn visit_references(
    access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe { (BASE.visit_references)(access, object, archive)?; }
    let player = object.cast::<UPlayer>();
    unsafe { archive.serialize_object_slot(ptr::addr_of_mut!((*player).actor).cast())?; }
    Ok(())
}

struct CoreSetPlayerOps<'a>(CoreAccess<'a>);

impl crate::actor::SetPlayerOps for CoreSetPlayerOps<'_> {
    type Error = RuntimeError;
    fn check_player(&mut self, valid: bool) -> RuntimeResult<()> {
        if valid { Ok(()) } else {
            Err(RuntimeError::new("InPlayer!=NULL").with_context("UnActor.cpp:1073"))
        }
    }
    unsafe fn player_actor(&mut self, player: *mut std::ffi::c_void) -> RuntimeResult<*mut std::ffi::c_void> {
        Ok(unsafe { (*player.cast::<UPlayer>()).actor })
    }
    unsafe fn set_actor(&mut self, player: *mut std::ffi::c_void, pawn: *mut std::ffi::c_void) -> RuntimeResult<()> {
        unsafe { (*player.cast::<UPlayer>()).actor = pawn; }
        Ok(())
    }
    unsafe fn possess(&mut self, pawn: *mut APlayerPawn) -> RuntimeResult<()> {
        let name = self.0.intern_str("Possess", hp2_core::name_owner::FindName::Find)?;
        let function = unsafe { self.0.find_object_function_by_name(pawn.cast(), name, false)? };
        if function.is_null() {
            return Err(RuntimeError::new("Possess function is missing"));
        }
        // FindFunctionChecked precedes AActor's begun-play gate.
        unsafe { crate::actor_owner::process_event(
            self.0, pawn.cast(), function, ptr::null_mut(), ptr::null_mut(),
        ) }
    }
    unsafe fn log_possessed(&mut self, pawn: *mut APlayerPawn) -> RuntimeResult<()> {
        let mut message = unsafe { self.0.object_full_name(pawn.cast())? };
        if message.last() == Some(&0) { message.pop(); }
        let prefix = hp2_core::lit!("Possessed PlayerPawn: ");
        message.splice(0..0, prefix[..prefix.len() - 1].iter().copied());
        let category = self.0.intern_str("Log", hp2_core::name_owner::FindName::Add)?;
        self.0.emit_log(category, &message)
    }
}

/// Publishes both player links, dispatches the checked Possess event, then logs.
///
/// # Safety
/// Both records belong to this Core owner. Pawn has loaded PlayerPawn script
/// metadata and a live LevelInfo when the checked Possess event is reached.
pub unsafe fn set_player(
    access: CoreAccess<'_>, pawn: *mut APlayerPawn, player: *mut UPlayer,
) -> RuntimeResult<()> {
    unsafe { crate::actor::set_player_with(&mut CoreSetPlayerOps(access), pawn, player.cast()) }
}
