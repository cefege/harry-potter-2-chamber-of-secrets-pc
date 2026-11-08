//! Canonical Rust-owned particle-list state for Engine particle systems.

use crate::particle::ParticleList;
use hp2_abi::engine::generated::{FBox, UObject, UParticleList, UParticleListPriv};
use hp2_abi::FVector;
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind};
use hp2_core::object_owner::ClassOps;
use std::alloc::Layout;
use std::ffi::c_void;
use std::ptr;

const OBJECT_OPS: ClassOps = NativeRecordKind::Object.operations();


/// The actor owner and native list record associated with one Rust particle arena.
pub(crate) struct ParticleListState {
    pub(crate) actor_owner: *mut c_void,
    pub(crate) particles: ParticleList,
}

impl ParticleListState {
    pub(crate) fn new(actor_owner: *mut c_void) -> Self {
        Self {
            actor_owner,
            particles: ParticleList::new(),
        }
    }
}

const LIST_OPS: ClassOps = {
    let mut ops = OBJECT_OPS;
    ops.construct = construct;
    ops.finalize_members = finalize_members;
    ops
};

/// Canonical Engine declaration for callback-free particle lists.
pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    NativeClassSpec {
        short_name: "ParticleList",
        package: "Engine",
        super_name: Some("Primitive"),
        within: "Object",
        flags: 1 | 0x0000_0008,
        object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<UParticleList>()),
        operations: Some(LIST_OPS),
        properties: &[],
        defaults: &[],
    },
    NativeClassSpec {
        short_name: "ParticleListPriv",
        package: "Engine",
        super_name: Some("ParticleList"),
        within: "Object",
        flags: 0x0000_0008,
        object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<UParticleListPriv>()),
        operations: Some(LIST_OPS),
        properties: &[],
        defaults: &[],
    },
];

/// # Safety
/// `object` must be a live Core-owned `UParticleList` allocation selected by its class record.
unsafe fn construct(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() {
        return Err(RuntimeError::new("Null canonical particle-list constructor target"));
    }
    // SAFETY: Core allocated the exact inherited UPrimitive prefix for this class.
    unsafe { crate::primitive_owner::construct_primitive(access, object)?; }
    let engine = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Particle-list construction has no canonical Engine state"))?;
    let mut lists = engine.particle_lists.borrow_mut();
    if lists.insert(object as usize, ParticleListState::new(ptr::null_mut())).is_some() {
        return Err(RuntimeError::new("Canonical particle-list identity was constructed twice"));
    }
    Ok(())
}

/// # Safety
/// `object` must be a live canonical particle list whose Engine state is available.
unsafe fn finalize_members(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() {
        return Err(RuntimeError::new("Null canonical particle-list finalization target"));
    }
    let engine = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Particle-list finalization has no canonical Engine state"))?;
    let state = engine.particle_lists.borrow_mut().remove(&(object as usize))
        .ok_or_else(|| RuntimeError::new("Canonical particle-list state is missing at finalization"))?;
    if !state.actor_owner.is_null() {
        let owner = state.actor_owner.cast::<hp2_abi::engine::generated::AParticleFX>();
        unsafe {
            if (*owner).particle_list == object.cast::<c_void>() {
                (*owner).particle_list = ptr::null_mut();
            }
        }
    }
    Ok(())
}


fn state_mut<'a>(access: CoreAccess<'a>, list: *mut c_void) -> RuntimeResult<std::cell::RefMut<'a, ParticleListState>> {
    if list.is_null() {
        return Err(RuntimeError::new("Null canonical particle-list identity"));
    }
    let engine = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Particle-list operation has no canonical Engine state"))?;
    let lists = engine.particle_lists.borrow_mut();
    if !lists.contains_key(&(list as usize)) {
        return Err(RuntimeError::new("Canonical particle-list state is missing"));
    }
    Ok(std::cell::RefMut::map(lists, |entries| entries.get_mut(&(list as usize)).expect("list checked above")))
}

pub(crate) fn start(access: CoreAccess<'_>, list: *mut c_void) -> RuntimeResult<*mut hp2_abi::engine::generated::UParticle> {
    Ok(state_mut(access, list)?.particles.start())
}

pub(crate) fn next(access: CoreAccess<'_>, list: *mut c_void) -> RuntimeResult<*mut hp2_abi::engine::generated::UParticle> {
    Ok(state_mut(access, list)?.particles.next_particle())
}

pub(crate) fn size(access: CoreAccess<'_>, list: *mut c_void) -> RuntimeResult<usize> {
    Ok(state_mut(access, list)?.particles.size())
}

pub(crate) fn remove_current(access: CoreAccess<'_>, list: *mut c_void) -> RuntimeResult<()> {
    state_mut(access, list)?.particles.remove_current();
    Ok(())
}

pub(crate) fn remove_all(access: CoreAccess<'_>, list: *mut c_void) -> RuntimeResult<()> {
    state_mut(access, list)?.particles.remove_all();
    Ok(())
}

pub(crate) fn add(access: CoreAccess<'_>, list: *mut c_void, particle: *const hp2_abi::engine::generated::UParticle) -> RuntimeResult<()> {
    if particle.is_null() {
        return Err(RuntimeError::new("Null particle added to canonical particle list"));
    }
    // SAFETY: callers provide a live native UParticle and the list copies its bytes.
    unsafe { state_mut(access, list)?.particles.add_native(particle); }
    Ok(())
}

pub(crate) fn remove_from_front(access: CoreAccess<'_>, list: *mut c_void) -> RuntimeResult<()> {
    state_mut(access, list)?.particles.remove_from_front();
    Ok(())
}

pub(crate) fn update_box(access: CoreAccess<'_>, list: *mut c_void) -> RuntimeResult<FBox> {
    let mut state = state_mut(access, list)?;
    let mut bounds: FBox = unsafe { std::mem::zeroed() };
    state.particles.update_box(&mut bounds);
    // SAFETY: the owner map admits only compiler-sized canonical particle lists.
    unsafe { ptr::addr_of_mut!((*list.cast::<UParticleList>()).world_box).write_unaligned(bounds); }
    Ok(bounds)
}

pub(crate) fn world_box(access: CoreAccess<'_>, list: *mut c_void) -> RuntimeResult<FBox> {
    state_mut(access, list)?;
    // SAFETY: successful state lookup establishes a live canonical list record.
    Ok(unsafe { ptr::addr_of!((*list.cast::<UParticleList>()).world_box).read_unaligned() })
}

pub(crate) fn bind_owner(access: CoreAccess<'_>, list: *mut c_void, actor: *mut c_void) -> RuntimeResult<()> {
    state_mut(access, list)?.actor_owner = actor;
    Ok(())
}

pub(crate) fn get_particle(access: CoreAccess<'_>, list: *mut c_void, id: i32) -> RuntimeResult<*mut hp2_abi::engine::generated::UParticle> {
    Ok(state_mut(access, list)?.particles.get_particle(id))
}
