//! Actor animation selection, skeletal queries, and link state from `UnScript.cpp`.

use std::ffi::c_void;

/// C++ keeps Unreal object access and virtual sequence lookup; Rust owns the
/// legacy-classic versus skeletal-animation selection order.
#[repr(C)]
pub struct Hp2ActorAnimServices {
    pub context: *mut c_void,
    pub mesh_is_skeletal:
        unsafe extern "C" fn(context: *mut c_void, mesh: *mut c_void, out: *mut i32) -> i32,
    pub actor_skeletal_animation: unsafe extern "C" fn(
        context: *mut c_void,
        actor: *mut c_void,
        out: *mut *mut c_void,
    ) -> i32,
    pub mesh_default_animation:
        unsafe extern "C" fn(context: *mut c_void, mesh: *mut c_void, out: *mut *mut c_void) -> i32,
    pub find_sequence: unsafe extern "C" fn(
        context: *mut c_void,
        owner: *mut c_void,
        sequence_name: *mut c_void,
        owner_kind: i32,
        out: *mut *mut c_void,
    ) -> i32,
}

const OWNER_MESH: i32 = 0;
const OWNER_SKELETAL_ANIMATION: i32 = 1;

/// Selects the object whose `GetAnimSeq` supplies `AActor::GetAnim`'s result.
///
/// Classic meshes search their own animation list. Skeletal meshes search the
/// actor's linked `SkelAnim`, falling back to the skeletal mesh's
/// `DefaultAnimation` only when no linked animation is present.
///
/// # Safety
/// The actor, mesh, sequence-name storage, output slot, and service table must
/// remain valid for the call. Service callbacks preserve C++ object semantics.
pub unsafe fn get_anim(
    actor: *mut c_void,
    mesh: *mut c_void,
    sequence_name: *mut c_void,
    out: *mut *mut c_void,
    services: &Hp2ActorAnimServices,
) -> i32 {
    if actor.is_null() || mesh.is_null() || sequence_name.is_null() || out.is_null() {
        return 1;
    }

    unsafe {
        let mut skeletal = 0;
        let status = (services.mesh_is_skeletal)(services.context, mesh, &mut skeletal);
        if status != 0 {
            return status;
        }
        if skeletal == 0 {
            return (services.find_sequence)(
                services.context,
                mesh,
                sequence_name,
                OWNER_MESH,
                out,
            );
        }

        let mut animation = std::ptr::null_mut();
        let status = (services.actor_skeletal_animation)(services.context, actor, &mut animation);
        if status != 0 {
            return status;
        }
        if animation.is_null() {
            let status = (services.mesh_default_animation)(services.context, mesh, &mut animation);
            if status != 0 {
                return status;
            }
        }
        (services.find_sequence)(
            services.context,
            animation,
            sequence_name,
            OWNER_SKELETAL_ANIMATION,
            out,
        )
    }
}

/// Reports whether the actor's selected animation source contains a sequence.
///
/// # Safety
/// Arguments and callbacks have the same validity requirements as [`get_anim`];
/// `out` points to a valid `i32`.
pub unsafe fn has_anim(
    actor: *mut c_void,
    mesh: *mut c_void,
    sequence_name: *mut c_void,
    out: *mut i32,
    services: &Hp2ActorAnimServices,
) -> i32 {
    if out.is_null() {
        return 1;
    }
    unsafe { *out = 0 };
    let mut sequence = std::ptr::null_mut();
    let status = unsafe { get_anim(actor, mesh, sequence_name, &mut sequence, services) };
    if status != 0 {
        return status;
    }
    unsafe { *out = i32::from(!sequence.is_null()) };
    0
}

/// C++ copies the selected sequence's `Group` FName into the output.
#[repr(C)]
pub struct Hp2ActorAnimGroupServices {
    pub context: *mut c_void,
    pub sequence_group:
        unsafe extern "C" fn(context: *mut c_void, sequence: *mut c_void, out: *mut c_void) -> i32,
}

/// Resolves an animation group, reporting whether a sequence was found.
///
/// # Safety
/// Arguments and lookup callbacks have the same validity requirements as
/// [`get_anim`]; `out_group` points to four writable FName bytes and
/// `out_found` points to a writable `i32`.
pub unsafe fn get_anim_group(
    actor: *mut c_void,
    mesh: *mut c_void,
    sequence_name: *mut c_void,
    out_group: *mut c_void,
    out_found: *mut i32,
    animation_services: &Hp2ActorAnimServices,
    group_services: &Hp2ActorAnimGroupServices,
) -> i32 {
    if out_group.is_null() || out_found.is_null() {
        return 1;
    }
    unsafe {
        std::ptr::write_bytes(out_group.cast::<u8>(), 0, std::mem::size_of::<i32>());
        *out_found = 0;
    }
    let mut sequence = std::ptr::null_mut();
    let status = unsafe {
        get_anim(
            actor,
            mesh,
            sequence_name,
            &mut sequence,
            animation_services,
        )
    };
    if status != 0 || sequence.is_null() {
        return status;
    }
    let status =
        unsafe { (group_services.sequence_group)(group_services.context, sequence, out_group) };
    if status == 0 {
        unsafe { *out_found = 1 };
    }
    status
}

/// C++ provides virtual mesh and auxiliary-channel access; Rust owns the
/// channel-selection order used by `AActor::execIsAnimating`.
#[repr(C)]
pub struct Hp2ActorAnimChannelServices {
    pub context: *mut c_void,
    pub auxiliary_count:
        unsafe extern "C" fn(context: *mut c_void, actor: *mut c_void, out: *mut i32) -> i32,
    pub auxiliary_bone: unsafe extern "C" fn(
        context: *mut c_void,
        actor: *mut c_void,
        index: i32,
        out: *mut i32,
    ) -> i32,
    pub auxiliary_is_animating: unsafe extern "C" fn(
        context: *mut c_void,
        actor: *mut c_void,
        index: i32,
        out: *mut i32,
    ) -> i32,
}

/// Selects a bone animation channel or the actor's base animation.
///
/// # Safety
/// `actor`, `out`, and `services` must remain valid for the call. The actor
/// must be a live `AActor`; service callbacks must preserve C++ semantics.
pub unsafe fn is_animating_channel(
    actor: *mut c_void,
    bone_is_none: i32,
    mesh_is_skeletal: i32,
    bone_index: i32,
    out: *mut i32,
    services: &Hp2ActorAnimChannelServices,
) -> i32 {
    if actor.is_null() || out.is_null() {
        return 1;
    }
    unsafe {
        if bone_is_none != 0 {
            *out = i32::from(crate::actor::is_animating(
                &*actor.cast::<hp2_abi::engine::generated::AActor>(),
            ));
            return 0;
        }
        *out = 0;
        if mesh_is_skeletal == 0 {
            return 0;
        }

        let mut count = 0;
        let status = (services.auxiliary_count)(services.context, actor, &mut count);
        if status != 0 {
            return status;
        }
        for index in 0..count {
            let mut channel_bone = 0;
            let status =
                (services.auxiliary_bone)(services.context, actor, index, &mut channel_bone);
            if status != 0 {
                return status;
            }
            if channel_bone == bone_index {
                let mut animating = 0;
                let status = (services.auxiliary_is_animating)(
                    services.context,
                    actor,
                    index,
                    &mut animating,
                );
                if status != 0 {
                    return status;
                }
                *out = animating;
                return 0;
            }
        }
        0
    }
}

/// Script-frame diagnostics for the animation query natives.
#[repr(C)]
pub struct Hp2ScriptAnimLogServices {
    pub context: *mut c_void,
    pub sequence_not_found: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *const c_void) -> i32,
    /// `which` is zero for GetAnimGroup and one for HasAnim.
    pub no_mesh: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> i32,
}

/// Decodes the source IsAnimating channel selectors before the shared search.
/// Providers are lazy: NAME_None does not require skeletal services.
/// # Safety
/// Actor/name/output and providers' graph records remain live synchronously.
pub unsafe fn exec_is_animating(
    actor: *mut c_void, bone: *mut c_void, out: *mut i32,
    bone_services: impl FnOnce() -> Result<Hp2ActorBoneNumberServices, i32>,
    channel_services: impl FnOnce() -> Result<Hp2ActorAnimChannelServices, i32>,
) -> i32 {
    unsafe {
        let none = i32::from(bone.cast::<u32>().read_unaligned() == 0);
        let mut skeletal = 0;
        let mut index = 0;
        if none == 0 {
            let mesh = (*actor.cast::<hp2_abi::engine::generated::AActor>()).mesh;
            if !mesh.is_null() {
                let services = match bone_services() { Ok(value) => value, Err(status) => return status };
                let status = (services.mesh_is_skeletal)(services.context, mesh, &mut skeletal);
                if status != 0 { return status; }
                if skeletal != 0 {
                    let status = (services.bone_index)(services.context, mesh, bone, &mut index);
                    if status != 0 { return status; }
                }
            }
        }
        let services = match channel_services() { Ok(value) => value, Err(status) => return status };
        is_animating_channel(actor, none, skeletal, index, out, &services)
    }
}

/// Executes GetAnimGroup's result and diagnostics with source effect ordering.
/// # Safety
/// Actor/name/frame/output and synchronous provider records remain live.
pub unsafe fn exec_get_anim_group(
    actor: *mut c_void, name: *mut c_void, frame: *mut c_void, out: *mut c_void,
    animation: impl FnOnce() -> Result<Hp2ActorAnimServices, i32>,
    group: impl FnOnce() -> Result<Hp2ActorAnimGroupServices, i32>,
    logging: impl FnOnce() -> Result<Hp2ScriptAnimLogServices, i32>,
) -> i32 {
    unsafe {
        let mesh = (*actor.cast::<hp2_abi::engine::generated::AActor>()).mesh;
        if mesh.is_null() {
            out.cast::<u32>().write_unaligned(0);
            let log = match logging() { Ok(value) => value, Err(status) => return status };
            return (log.no_mesh)(log.context, frame, 0);
        }
        let animation = match animation() { Ok(value) => value, Err(status) => return status };
        let group = match group() { Ok(value) => value, Err(status) => return status };
        let mut found = 0;
        let status = get_anim_group(actor, mesh, name, out, &mut found, &animation, &group);
        if status != 0 { return status; }
        if found == 0 {
            let log = match logging() { Ok(value) => value, Err(status) => return status };
            return (log.sequence_not_found)(log.context, frame, mesh, name.cast_const());
        }
        0
    }
}

/// Executes HasAnim; a missing mesh logs without writing the caller's result.
/// # Safety
/// Actor/name/frame/output and synchronous provider records remain live.
pub unsafe fn exec_has_anim(
    actor: *mut c_void, name: *mut c_void, frame: *mut c_void, out: *mut c_void,
    animation: impl FnOnce() -> Result<Hp2ActorAnimServices, i32>,
    logging: impl FnOnce() -> Result<Hp2ScriptAnimLogServices, i32>,
) -> i32 {
    unsafe {
        let mesh = (*actor.cast::<hp2_abi::engine::generated::AActor>()).mesh;
        if mesh.is_null() {
            let log = match logging() { Ok(value) => value, Err(status) => return status };
            return (log.no_mesh)(log.context, frame, 1);
        }
        let animation = match animation() { Ok(value) => value, Err(status) => return status };
        let mut found = 0;
        let status = has_anim(actor, mesh, name, &mut found, &animation);
        if status != 0 { return status; }
        out.cast::<u32>().write_unaligned(found as u32);
        0
    }
}

/// C++ services for the `USkeletalMesh::BoneIndex` script query.
#[repr(C)]
pub struct Hp2ActorBoneNumberServices {
    pub context: *mut c_void,
    pub mesh_is_skeletal:
        unsafe extern "C" fn(context: *mut c_void, mesh: *mut c_void, out: *mut i32) -> i32,
    pub bone_index: unsafe extern "C" fn(
        context: *mut c_void,
        mesh: *mut c_void,
        bone_name: *mut c_void,
        out: *mut i32,
    ) -> i32,
}

/// Returns a skeletal mesh bone index, or zero when the actor mesh is not skeletal.
///
/// # Safety
/// `mesh`, `bone_name`, `out`, and `services` must remain valid for the call;
/// callbacks must preserve C++ object semantics.
pub unsafe fn bone_number(
    mesh: *mut c_void,
    bone_name: *mut c_void,
    out: *mut i32,
    services: &Hp2ActorBoneNumberServices,
) -> i32 {
    if out.is_null() {
        return 1;
    }
    unsafe { *out = 0 };
    if mesh.is_null() || bone_name.is_null() {
        return 0;
    }

    unsafe {
        let mut skeletal = 0;
        let status = (services.mesh_is_skeletal)(services.context, mesh, &mut skeletal);
        if status != 0 {
            return status;
        }
        if skeletal == 0 {
            return 0;
        }
        (services.bone_index)(services.context, mesh, bone_name, out)
    }
}

/// C++ services for the `USkeletalMesh::BoneName` script query.
#[repr(C)]
pub struct Hp2ActorBoneNameServices {
    pub context: *mut c_void,
    pub mesh_is_skeletal:
        unsafe extern "C" fn(context: *mut c_void, mesh: *mut c_void, out: *mut i32) -> i32,
    pub bone_name: unsafe extern "C" fn(
        context: *mut c_void,
        mesh: *mut c_void,
        bone_number: i32,
        out: *mut c_void,
    ) -> i32,
}

/// Resolves a bone name for a skeletal mesh, otherwise writes `NAME_None`.
///
/// # Safety
/// `out` points to a four-byte `FName`; `mesh`, `out`, and `services` must
/// remain valid for the call. Callbacks preserve C++ object semantics.
pub unsafe fn bone_name(
    mesh: *mut c_void,
    bone_number: i32,
    out: *mut c_void,
    services: &Hp2ActorBoneNameServices,
) -> i32 {
    if out.is_null() {
        return 1;
    }
    unsafe { std::ptr::write_bytes(out.cast::<u8>(), 0, std::mem::size_of::<i32>()) };
    if mesh.is_null() {
        return 0;
    }

    unsafe {
        let mut skeletal = 0;
        let status = (services.mesh_is_skeletal)(services.context, mesh, &mut skeletal);
        if status != 0 {
            return status;
        }
        if skeletal == 0 {
            return 0;
        }
        (services.bone_name)(services.context, mesh, bone_number, out)
    }
}

/// C++ services for actor bone position and rotation script queries.
#[repr(C)]
pub struct Hp2ActorBoneTransformServices {
    pub context: *mut c_void,
    pub mesh_is_skeletal:
        unsafe extern "C" fn(context: *mut c_void, mesh: *mut c_void, out: *mut i32) -> i32,
    pub bone_index: unsafe extern "C" fn(
        context: *mut c_void,
        mesh: *mut c_void,
        bone_name: *mut c_void,
        out: *mut i32,
    ) -> i32,
    pub bone_position: unsafe extern "C" fn(
        context: *mut c_void,
        actor: *mut c_void,
        mesh: *mut c_void,
        bone_index: i32,
        out: *mut c_void,
    ) -> i32,
    pub bone_rotation: unsafe extern "C" fn(
        context: *mut c_void,
        actor: *mut c_void,
        mesh: *mut c_void,
        bone_index: i32,
        out: *mut c_void,
    ) -> i32,
}

pub const BONE_TRANSFORM_POSITION: i32 = 0;
pub const BONE_TRANSFORM_ROTATION: i32 = 1;

/// Selects a skeletal bone transform or the actor's location/rotation fallback.
///
/// # Safety
/// `actor`, `out`, and `services` must remain valid; the actor must be a live
/// `AActor`. `out` points to an `FVector` for position or `FRotator` for
/// rotation.
pub unsafe fn bone_transform(
    actor: *mut c_void,
    mesh: *mut c_void,
    bone_name: *mut c_void,
    transform_kind: i32,
    out: *mut c_void,
    services: &Hp2ActorBoneTransformServices,
) -> i32 {
    if actor.is_null() || out.is_null() {
        return 1;
    }
    if transform_kind != BONE_TRANSFORM_POSITION && transform_kind != BONE_TRANSFORM_ROTATION {
        return 1;
    }

    unsafe {
        let actor_ref = &*actor.cast::<hp2_abi::engine::generated::AActor>();
        if transform_kind == BONE_TRANSFORM_POSITION {
            *out.cast::<hp2_abi::FVector>() = actor_ref.location;
        } else {
            *out.cast::<hp2_abi::engine::generated::FRotator>() = actor_ref.rotation;
        }
        if mesh.is_null() || bone_name.is_null() {
            return 0;
        }

        let mut skeletal = 0;
        let status = (services.mesh_is_skeletal)(services.context, mesh, &mut skeletal);
        if status != 0 {
            return status;
        }
        if skeletal == 0 {
            return 0;
        }
        let mut bone_index = 0;
        let status = (services.bone_index)(services.context, mesh, bone_name, &mut bone_index);
        if status != 0 {
            return status;
        }
        let callback = if transform_kind == BONE_TRANSFORM_POSITION {
            services.bone_position
        } else {
            services.bone_rotation
        };
        callback(services.context, actor, mesh, bone_index, out)
    }
}

/// Assigns the actor's linked skeletal animation, including clearing it.
///
/// # Safety
/// `actor` must be null or point to a live `AActor`-layout object.
pub unsafe fn link_skeletal_animation(actor: *mut c_void, animation: *mut c_void) -> i32 {
    let Some(actor) = (unsafe { actor.cast::<hp2_abi::engine::generated::AActor>().as_mut() })
    else {
        return 1;
    };
    actor.skel_anim = animation;
    0
}

/// Selects an auxiliary animation channel for `AActor::execFinishAnim` and
/// schedules its latent completion action.
///
/// # Safety
/// `actor`, `bone_name`, and both service tables must remain valid. The actor's
/// auxiliary animation pointers must be live `AActor`-layout channels.
pub unsafe fn exec_finish_anim(
    actor: *mut c_void,
    bone_name: *mut c_void,
    finish_action: i32,
    bone_services: &Hp2ActorBoneNumberServices,
    latent_services: &crate::actor::Hp2ActorLatentServices,
) -> i32 {
    use hp2_abi::engine::generated::AActor;

    if actor.is_null() || bone_name.is_null() {
        return 1;
    }
    unsafe {
        let mut channel = actor.cast::<AActor>();
        if bone_name.cast::<u32>().read_unaligned() != 0 {
            let mesh = (*channel).mesh;
            if mesh.is_null() {
                return 0;
            }
            let mut skeletal = 0;
            let status =
                (bone_services.mesh_is_skeletal)(bone_services.context, mesh, &mut skeletal);
            if status != 0 {
                return status;
            }
            if skeletal == 0 {
                return 0;
            }
            let mut bone_index = 0;
            let status =
                (bone_services.bone_index)(bone_services.context, mesh, bone_name, &mut bone_index);
            if status != 0 {
                return status;
            }
            let aux = std::ptr::addr_of!((*channel).aux_anims).read_unaligned();
            if aux.num > 0 && aux.data.is_null() {
                return 1;
            }
            let channels = aux.data.cast::<*mut c_void>();
            channel = std::ptr::null_mut();
            for i in 0..aux.num {
                let candidate = channels.add(i as usize).read().cast::<AActor>();
                if (*candidate).anim_bone as i32 == bone_index {
                    channel = candidate;
                    break;
                }
            }
            if channel.is_null() {
                return 0;
            }
        }

        if (*channel).b_anim_loop() {
            (*channel).set_b_anim_loop(false);
            (*channel).set_b_anim_finished(false);
        }
        if crate::actor::is_animating(&*channel) && (*channel).anim_frame < (*channel).anim_last {
            (latent_services.set_latent_action)(
                latent_services.context,
                channel.cast(),
                finish_action,
            )
        } else {
            0
        }
    }
}

/// Engine-only effects used by `AActor::PlayAnim`. Callback statuses are
/// independent of their output values and preserve pending C++ exceptions.
#[repr(C)]
pub struct Hp2ActorPlayAnimServices {
    pub context: *mut c_void,
    pub animation: *const Hp2ActorAnimServices,
    pub root_is_move: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    pub bone_index: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut i32) -> i32,
    /// This packed flag is not exposed by the actor ABI mirror.
    pub channel_not_replaceable: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    pub load_channel_class: unsafe extern "C" fn(*mut c_void) -> i32,
    pub create_channel:
        unsafe extern "C" fn(*mut c_void, *mut c_void, i32, *mut c_void, *mut *mut c_void) -> i32,
    /// Destroy the indexed channel, then call engine TArray::Remove, preserving
    /// engine-owned storage shrinking. Destruction's UBOOL is ignored.
    pub destroy_channel: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> i32,
    /// Recursive dispatch uses PlayAnim's default AT_Replace and NAME_None.
    pub play_channel: unsafe extern "C" fn(
        *mut c_void,
        *mut c_void,
        *mut c_void,
        i32,
        f32,
        f32,
        f32,
        *mut i32,
    ) -> i32,
    /// `missing_mesh` selects the original no-mesh versus missing-sequence log.
    pub log: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, i32) -> i32,
}
/// Engine effects for skeletal auxiliary-channel creation.
#[repr(C)]
pub struct Hp2ActorCreateAnimChannelServices {
    pub context: *mut c_void,
    pub mesh_is_skeletal: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    pub bone_index: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut i32) -> i32,
    pub channel_not_replaceable: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    /// Spawns a channel and initializes its mesh, linked animation, bone, and flags.
    pub spawn_channel: unsafe extern "C" fn(
        *mut c_void,
        *mut c_void,
        *mut c_void,
        i32,
        i32,
        i32,
        *mut *mut c_void,
    ) -> i32,
    /// Destroys the indexed channel and removes it from the owner's TArray.
    pub destroy_channel: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> i32,
    pub insert_channel: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
    pub append_channel: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
}

/// Reads `SMesh->RefSkeleton(Bone2).NumChildren` directly through the ABI
/// mirror, replicating `static inline bool IsSubset(...)` from
/// `Engine/Src/UnScript.cpp`. Matches the original's unchecked
/// `TArray::operator()` index: no bounds check beyond what the engine
/// guarantees for a valid skeletal mesh and in-range bone index.
///
/// # Safety
/// `mesh` must be a live `USkeletalMesh*` whose `RefSkeleton` has at least
/// `bone2 + 1` entries.
unsafe fn bone_is_subset(mesh: *mut c_void, bone1: i32, bone2: i32) -> bool {
    use hp2_abi::engine::generated::{FMeshBone, USkeletalMesh};
    unsafe {
        let bones = (*mesh.cast::<USkeletalMesh>())
            .ref_skeleton
            .data
            .cast::<FMeshBone>();
        let num_children = (*bones.add(bone2 as usize)).num_children;
        bone1 >= bone2 && bone1 < bone2 + num_children
    }
}

/// Creates or reuses a skeletal auxiliary animation channel.
///
/// Rust owns channel selection and Replace/Combine policy. C++ callbacks
/// preserve Unreal mesh queries, object spawning, destruction and TArray
/// mutation semantics.
///
/// # Safety
/// The actor, class, root-bone FName and service table remain valid.
pub unsafe fn create_anim_channel(
    actor: *mut c_void,
    new_class: *mut c_void,
    animation_type: i32,
    root_bone: *mut c_void,
    transient: i32,
    not_replaceable: i32,
    out: *mut *mut c_void,
    services: &Hp2ActorCreateAnimChannelServices,
) -> i32 {
    use hp2_abi::engine::generated::AActor;
    if actor.is_null() || root_bone.is_null() || out.is_null() {
        return 1;
    }
    unsafe {
        out.write(std::ptr::null_mut());
        let actor = actor.cast::<AActor>();
        let mesh = (*actor).mesh;
        if mesh.is_null() {
            return 0;
        }
        let mut skeletal = 0;
        let status = (services.mesh_is_skeletal)(services.context, mesh, &mut skeletal);
        if status != 0 {
            return status;
        }
        if skeletal == 0 || root_bone.cast::<u32>().read_unaligned() == 0 {
            return 0;
        }
        let mut bone_start = -1;
        let status = (services.bone_index)(services.context, mesh, root_bone, &mut bone_start);
        if status != 0 {
            return status;
        }
        if bone_start < 0 {
            return 0;
        }

        let mut aux = std::ptr::addr_of!((*actor).aux_anims).read_unaligned();
        if aux.num > 0 && aux.data.is_null() {
            return 1;
        }
        let channels = aux.data.cast::<*mut AActor>();
        for index in 0..aux.num {
            let channel = channels.add(index as usize).read_unaligned();
            if channel.is_null()
                || (*channel).anim_bone as i32 != bone_start
                || (*channel).b_anim_transient() != (transient != 0)
            {
                continue;
            }
            let mut protected = 0;
            let status = (services.channel_not_replaceable)(
                services.context,
                channel.cast(),
                &mut protected,
            );
            if status != 0 {
                return status;
            }
            if (protected != 0) == (not_replaceable != 0) {
                out.write(channel.cast());
                return 0;
            }
        }

        let mut channel = std::ptr::null_mut();
        let status = (services.spawn_channel)(
            services.context,
            actor.cast(),
            new_class,
            bone_start,
            transient,
            not_replaceable,
            &mut channel,
        );
        if status != 0 || channel.is_null() {
            return status;
        }
        aux = std::ptr::addr_of!((*actor).aux_anims).read_unaligned();
        if animation_type == 1 {
            let status = (services.insert_channel)(services.context, actor.cast(), channel);
            if status == 0 {
                out.write(channel);
            }
            return status;
        }

        let mut index = aux.num - 1;
        while index >= 0 {
            aux = std::ptr::addr_of!((*actor).aux_anims).read_unaligned();
            let existing = aux
                .data
                .cast::<*mut AActor>()
                .add(index as usize)
                .read_unaligned();
            let subset = unsafe { bone_is_subset(mesh, (*existing).anim_bone as i32, bone_start) };
            if subset {
                let mut protected = 0;
                let status = (services.channel_not_replaceable)(
                    services.context,
                    existing.cast(),
                    &mut protected,
                );
                if status != 0 {
                    return status;
                }
                if protected == 0 {
                    let status = (services.destroy_channel)(services.context, actor.cast(), index);
                    if status != 0 {
                        return status;
                    }
                }
            }
            index -= 1;
        }
        let status = (services.append_channel)(services.context, actor.cast(), channel);
        if status == 0 {
            out.write(channel);
        }
        status
    }
}

/// Dispatches decoded script arguments to `AActor::CreateAnimChannel`.
///
/// # Safety
/// The actor, class, root FName and services remain valid.
pub unsafe fn exec_create_anim_channel(
    actor: *mut c_void,
    new_class: *mut c_void,
    animation_type: i32,
    root_bone: *mut c_void,
    transient: i32,
    not_replaceable: i32,
    out: *mut *mut c_void,
    services: &Hp2ActorCreateAnimChannelServices,
) -> i32 {
    unsafe {
        create_anim_channel(
            actor,
            new_class,
            animation_type,
            root_bone,
            transient,
            not_replaceable,
            out,
            services,
        )
    }
}

#[cfg(test)]
mod create_anim_channel_tests {
    use super::*;
    use hp2_abi::engine::generated::{AActor, FMeshBone, TArrayRaw, USkeletalMesh};
    use std::ptr;

    struct Context {
        skeletal: bool,
        bone: i32,
        protected: Vec<usize>,
        spawned: *mut c_void,
        events: Vec<&'static str>,
        destroyed: Vec<i32>,
        fail: &'static str,
    }

    unsafe fn emit(context: *mut c_void, event: &'static str) -> (*mut Context, i32) {
        let context = context.cast::<Context>();
        unsafe { (*context).events.push(event) };
        let status = unsafe { i32::from((*context).fail == event) * 91 };
        (context, status)
    }

    unsafe extern "C" fn skeletal(context: *mut c_void, _: *mut c_void, out: *mut i32) -> i32 {
        let (context, status) = unsafe { emit(context, "skeletal") };
        unsafe { out.write(i32::from((*context).skeletal)) };
        status
    }

    unsafe extern "C" fn bone(
        context: *mut c_void,
        _: *mut c_void,
        _: *mut c_void,
        out: *mut i32,
    ) -> i32 {
        let (context, status) = unsafe { emit(context, "bone") };
        unsafe { out.write((*context).bone) };
        status
    }

    unsafe extern "C" fn protected(
        context: *mut c_void,
        channel: *mut c_void,
        out: *mut i32,
    ) -> i32 {
        let (context, status) = unsafe { emit(context, "protected") };
        let found = unsafe { (*context).protected.contains(&(channel as usize)) };
        unsafe { out.write(i32::from(found)) };
        status
    }

    unsafe extern "C" fn spawn(
        context: *mut c_void,
        _: *mut c_void,
        _: *mut c_void,
        _: i32,
        _: i32,
        _: i32,
        out: *mut *mut c_void,
    ) -> i32 {
        let (context, status) = unsafe { emit(context, "spawn") };
        unsafe { out.write((*context).spawned) };
        status
    }

    unsafe extern "C" fn destroy(context: *mut c_void, actor: *mut c_void, index: i32) -> i32 {
        let (context, status) = unsafe { emit(context, "destroy") };
        if status != 0 {
            return status;
        }
        unsafe {
            (*context).destroyed.push(index);
            let actor = actor.cast::<AActor>();
            let mut aux = ptr::addr_of!((*actor).aux_anims).read_unaligned();
            let data = aux.data.cast::<*mut AActor>();
            ptr::copy(
                data.add(index as usize + 1),
                data.add(index as usize),
                (aux.num - index - 1) as usize,
            );
            aux.num -= 1;
            ptr::addr_of_mut!((*actor).aux_anims).write_unaligned(aux);
        }
        0
    }

    unsafe extern "C" fn insert(
        context: *mut c_void,
        actor: *mut c_void,
        channel: *mut c_void,
    ) -> i32 {
        let (_, status) = unsafe { emit(context, "insert") };
        if status != 0 {
            return status;
        }
        unsafe {
            let actor = actor.cast::<AActor>();
            let mut aux = ptr::addr_of!((*actor).aux_anims).read_unaligned();
            let data = aux.data.cast::<*mut AActor>();
            ptr::copy(data, data.add(1), aux.num as usize);
            data.write(channel.cast());
            aux.num += 1;
            ptr::addr_of_mut!((*actor).aux_anims).write_unaligned(aux);
        }
        0
    }

    unsafe extern "C" fn append(
        context: *mut c_void,
        actor: *mut c_void,
        channel: *mut c_void,
    ) -> i32 {
        let (_, status) = unsafe { emit(context, "append") };
        if status != 0 {
            return status;
        }
        unsafe {
            let actor = actor.cast::<AActor>();
            let mut aux = ptr::addr_of!((*actor).aux_anims).read_unaligned();
            aux.data
                .cast::<*mut AActor>()
                .add(aux.num as usize)
                .write(channel.cast());
            aux.num += 1;
            ptr::addr_of_mut!((*actor).aux_anims).write_unaligned(aux);
        }
        0
    }

    struct Case {
        actor: Box<AActor>,
        slots: Box<[*mut AActor; 8]>,
        channels: Vec<Box<AActor>>,
        spawned: Box<AActor>,
        _mesh: Box<USkeletalMesh>,
        _bones: Box<[FMeshBone; 6]>,
        context: Context,
    }

    impl Case {
        fn new(bone: i32) -> Self {
            // `bones[5].num_children = 3` reproduces this harness's former
            // mock `bone_is_subset` range (`child >= parent && child <
            // parent + 3`) for the one bone (5) the tests drive through
            // `ref_skeleton`, now that `bone_is_subset` reads a real
            // `USkeletalMesh::RefSkeleton` TArray instead of a callback.
            let mut bones: Box<[FMeshBone; 6]> = Box::new(unsafe { std::mem::zeroed() });
            bones[5].num_children = 3;
            let mut mesh: Box<USkeletalMesh> = Box::new(unsafe { std::mem::zeroed() });
            mesh.ref_skeleton = TArrayRaw {
                data: bones.as_mut_ptr().cast(),
                num: bones.len() as i32,
                max: bones.len() as i32,
            };
            let mut actor: Box<AActor> = Box::new(unsafe { std::mem::zeroed() });
            actor.mesh = (&mut *mesh as *mut USkeletalMesh).cast();
            let mut slots = Box::new([ptr::null_mut::<AActor>(); 8]);
            actor.aux_anims = TArrayRaw {
                data: slots.as_mut_ptr().cast(),
                num: 0,
                max: 8,
            };
            let mut spawned: Box<AActor> = Box::new(unsafe { std::mem::zeroed() });
            spawned.anim_bone = 99;
            Self {
                actor,
                slots,
                channels: Vec::new(),
                _mesh: mesh,
                _bones: bones,
                context: Context {
                    skeletal: true,
                    bone,
                    protected: Vec::new(),
                    spawned: (&mut *spawned as *mut AActor).cast(),
                    events: Vec::new(),
                    destroyed: Vec::new(),
                    fail: "",
                },
                spawned,
            }
        }

        fn add_channel(&mut self, bone: u8, transient: bool, protected: bool) -> *mut AActor {
            let mut channel: Box<AActor> = Box::new(unsafe { std::mem::zeroed() });
            channel.anim_bone = bone;
            channel.set_b_anim_transient(transient);
            let pointer = &mut *channel as *mut AActor;
            if protected {
                self.context.protected.push(pointer as usize);
            }
            let index = self.actor.aux_anims.num as usize;
            self.slots[index] = pointer;
            self.actor.aux_anims.num += 1;
            self.channels.push(channel);
            pointer
        }

        fn run(
            &mut self,
            kind: i32,
            root: u32,
            transient: i32,
            protected: i32,
        ) -> (i32, *mut c_void) {
            let context = (&mut self.context as *mut Context).cast();
            let services = Hp2ActorCreateAnimChannelServices {
                context,
                mesh_is_skeletal: skeletal,
                bone_index: bone,
                channel_not_replaceable: self::protected,
                spawn_channel: spawn,
                destroy_channel: destroy,
                insert_channel: insert,
                append_channel: append,
            };
            let mut name = root;
            let mut out = ptr::null_mut();
            let status = unsafe {
                create_anim_channel(
                    (&mut *self.actor as *mut AActor).cast(),
                    0x2000usize as *mut c_void,
                    kind,
                    (&mut name as *mut u32).cast(),
                    transient,
                    protected,
                    &mut out,
                    &services,
                )
            };
            (status, out)
        }

        fn channel_at(&self, index: usize) -> *mut AActor {
            self.slots[index]
        }
    }

    #[test]
    fn reuses_only_channels_with_matching_bone_transient_and_protection_flags() {
        let mut case = Case::new(4);
        case.add_channel(4, false, false);
        case.add_channel(4, true, true);
        let matching = case.add_channel(4, true, false);
        assert_eq!(case.run(0, 10, 1, 0), (0, matching.cast()));
        assert_eq!(case.actor.aux_anims.num, 3);
        assert!(!case.context.events.contains(&"spawn"));
        assert_eq!(
            case.context.events,
            ["skeletal", "bone", "protected", "protected"]
        );
    }

    #[test]
    fn combine_inserts_new_channel_at_front_without_subset_cleanup() {
        let mut case = Case::new(5);
        let old = case.add_channel(6, false, false);
        let spawned = (&mut *case.spawned as *mut AActor).cast();
        assert_eq!(case.run(1, 10, 0, 0), (0, spawned));
        assert_eq!(case.actor.aux_anims.num, 2);
        assert_eq!(case.channel_at(0).cast(), spawned);
        assert_eq!(case.channel_at(1), old);
        assert_eq!(case.context.events.last(), Some(&"insert"));
    }

    #[test]
    fn replace_removes_only_unprotected_channels_in_the_bone_subset_then_appends() {
        let mut case = Case::new(5);
        case.add_channel(6, false, false);
        let protected = case.add_channel(7, false, true);
        let upper = case.add_channel(8, false, false);
        let outside = case.add_channel(9, false, false);
        let spawned_actor = &mut *case.spawned as *mut AActor;
        let spawned = spawned_actor.cast();
        assert_eq!(case.run(0, 10, 0, 0), (0, spawned));
        assert_eq!(case.context.destroyed, [0]);
        assert_eq!(case.actor.aux_anims.num, 4);
        assert_eq!(
            (0..4)
                .map(|index| case.channel_at(index))
                .collect::<Vec<_>>(),
            [protected, upper, outside, spawned_actor],
        );
    }

    #[test]
    fn rejects_missing_skeletal_bone_and_propagates_service_failures() {
        let mut case = Case::new(-1);
        assert_eq!(case.run(0, 10, 0, 0), (0, ptr::null_mut()));
        assert!(!case.context.events.contains(&"spawn"));
        case.context.bone = 4;
        case.context.fail = "bone";
        case.context.events.clear();
        assert_eq!(case.run(0, 10, 0, 0), (91, ptr::null_mut()));
        assert_eq!(case.context.events, ["skeletal", "bone"]);
        case.context.fail = "";
        case.context.events.clear();
        case.context.skeletal = false;
        assert_eq!(case.run(0, 10, 0, 0), (0, ptr::null_mut()));
        assert_eq!(case.context.events, ["skeletal"]);
        case.context.events.clear();
        case.context.skeletal = true;
        assert_eq!(case.run(0, 0, 0, 0), (0, ptr::null_mut()));
        assert_eq!(case.context.events, ["skeletal"]);
    }
}

/// Clears replaceable auxiliary channels in reverse order. No Rust storage or
/// actor references survive callbacks: destruction can reallocate the array.
unsafe fn clear_anim_channels(
    actor: *mut hp2_abi::engine::generated::AActor,
    mesh: *mut c_void,
    bone_start: Option<i32>,
    services: &Hp2ActorPlayAnimServices,
) -> i32 {
    unsafe {
        let mut i = (*actor).aux_anims.num - 1;
        while i >= 0 {
            let aux = std::ptr::addr_of!((*actor).aux_anims).read_unaligned();
            let channel = aux
                .data
                .cast::<*mut hp2_abi::engine::generated::AActor>()
                .add(i as usize)
                .read_unaligned();
            if let Some(bone_start) = bone_start {
                let subset = unsafe { bone_is_subset(mesh, (*channel).anim_bone as i32, bone_start) };
                if !subset {
                    i -= 1;
                    continue;
                }
            }
            let mut protected = 0;
            let status = (services.channel_not_replaceable)(
                services.context,
                channel.cast(),
                &mut protected,
            );
            if status != 0 {
                return status;
            }
            if protected == 0 {
                if (*channel).b_anim_transient() {
                    let status = (services.destroy_channel)(services.context, actor.cast(), i);
                    if status != 0 {
                        return status;
                    }
                } else {
                    (*channel).anim_sequence = [0; 4];
                }
            }
            i -= 1;
        }
    }
    0
}

/// Runs the complete legacy `AActor::PlayAnim` state transition.
///
/// # Safety
/// The actor and auxiliary channels must be live ABI-layout objects; names
/// point to FNames, `out` to an i32, and both service tables remain valid.
/// Callbacks must preserve engine semantics and contain C++ exceptions.
pub unsafe fn play_anim(
    actor: *mut c_void,
    sequence_name: *mut c_void,
    looping: i32,
    play_rate: f32,
    tween_time: f32,
    min_rate: f32,
    animation_type: i32,
    root_bone: *mut c_void,
    out: *mut i32,
    services: &Hp2ActorPlayAnimServices,
) -> i32 {
    use hp2_abi::engine::generated::{AActor, FMeshAnimSeq};
    if actor.is_null()
        || sequence_name.is_null()
        || root_bone.is_null()
        || out.is_null()
        || services.animation.is_null()
    {
        return 1;
    }
    unsafe {
        out.write(0);
        let actor = actor.cast::<AActor>();
        let mesh = (*actor).mesh;
        if mesh.is_null() {
            return (services.log)(services.context, actor.cast(), sequence_name, 1);
        }
        let animation = &*services.animation;
        let mut skeletal = 0;
        let status = (animation.mesh_is_skeletal)(animation.context, mesh, &mut skeletal);
        if status != 0 {
            return status;
        }
        let mut is_move = 0;
        let status = (services.root_is_move)(services.context, root_bone, &mut is_move);
        if status != 0 {
            return status;
        }
        (*actor).set_b_anim_move(is_move != 0);
        let sequence = sequence_name.cast::<[u8; 4]>().read_unaligned();
        if skeletal != 0 && is_move == 0 && root_bone.cast::<u32>().read_unaligned() != 0 {
            if sequence != [0; 4] {
                let status = (services.load_channel_class)(services.context);
                if status != 0 {
                    return status;
                }
                let mut channel = std::ptr::null_mut();
                let status = (services.create_channel)(
                    services.context,
                    actor.cast(),
                    animation_type,
                    root_bone,
                    &mut channel,
                );
                if status != 0 {
                    return status;
                }
                if !channel.is_null() {
                    return (services.play_channel)(
                        services.context,
                        channel,
                        sequence_name,
                        looping,
                        play_rate,
                        tween_time,
                        min_rate,
                        out,
                    );
                }
                // Failed channel creation falls through to whole-skeleton playback.
            } else {
                let mut bone_start = 0;
                let status =
                    (services.bone_index)(services.context, mesh, root_bone, &mut bone_start);
                if status != 0 {
                    return status;
                }
                if bone_start < 0 {
                    return 0;
                }
                let status = clear_anim_channels(actor, mesh, Some(bone_start), services);
                if status != 0 {
                    return status;
                }
                out.write(1);
                return 0;
            }
        }
        if animation_type == 0 {
            let status = clear_anim_channels(actor, mesh, None, services);
            if status != 0 {
                return status;
            }
        }
        let mut seq = std::ptr::null_mut();
        let status = get_anim(
            actor.cast(),
            (*actor).mesh,
            sequence_name,
            &mut seq,
            animation,
        );
        if status != 0 {
            return status;
        }
        if seq.is_null() && sequence != [0; 4] {
            return (services.log)(services.context, actor.cast(), sequence_name, 0);
        }
        let looping = looping != 0;
        if !seq.is_null() {
            let seq = &*seq.cast::<FMeshAnimSeq>();
            let frames = seq.num_frames as f32;
            if (*actor).anim_sequence == sequence
                && looping
                && (*actor).b_anim_loop()
                && crate::actor::is_animating(&*actor)
            {
                (*actor).anim_rate = play_rate * seq.rate / frames;
                (*actor).set_b_anim_finished(false);
                (*actor).anim_min_rate = if min_rate != 0.0 {
                    min_rate * (seq.rate / frames)
                } else {
                    0.0
                };
                let old = std::ptr::addr_of!((*actor).sim_anim).read_unaligned();
                let mut sim = old;
                sim.y = 5000.0 * (*actor).anim_rate;
                sim.w = -10000.0 * (1.0 - 1.0 / frames);
                if old == sim {
                    sim.w += 1.0;
                }
                (*actor).sim_anim = sim;
                out.write(1);
                return 0;
            }
            (*actor).anim_rate = play_rate * seq.rate / frames;
            (*actor).anim_last = 1.0 - 1.0 / frames;
            (*actor).anim_min_rate = if min_rate != 0.0 {
                min_rate * (seq.rate / frames)
            } else {
                0.0
            };
            (*actor).set_b_anim_notify(seq.notifys.num != 0);
            (*actor).set_b_anim_finished(false);
            (*actor).set_b_anim_loop(looping);
        } else {
            (*actor).anim_last = 0.0;
            (*actor).set_b_anim_loop(false);
        }
        (*actor).anim_sequence = sequence;
        (*actor).set_b_anim_finished(false);
        (*actor).anim_frame = 0.0;
        (*actor).tween_alpha = 0.0;
        if (*actor).anim_last == 0.0 {
            (*actor).anim_min_rate = 0.0;
            (*actor).anim_rate = 0.0;
            (*actor).set_b_anim_notify(false);
        }
        if tween_time > 0.0 {
            (*actor).tween_rate = 1.0 / tween_time;
        } else if tween_time < 0.0 {
            (*actor).tween_rate = 2.0;
        } else {
            (*actor).tween_rate = 0.0;
            (*actor).tween_alpha = 1.0;
        }
        let old = std::ptr::addr_of!((*actor).sim_anim).read_unaligned();
        let mut sim = hp2_abi::FPlane {
            x: 10000.0 * (*actor).anim_frame,
            y: 5000.0 * (*actor).anim_rate,
            z: 1000.0 * (*actor).tween_rate,
            w: -10000.0 * (*actor).anim_last,
        };
        if sim.y > 32767.0 {
            sim.y = 32767.0;
        }
        if !looping && old == sim {
            sim.w += 1.0;
        }
        (*actor).sim_anim = sim;
        out.write(1);
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACTOR: usize = 0x1000;
    const MESH: usize = 0x2000;
    const SKELETAL_ANIMATION: usize = 0x3000;
    const DEFAULT_ANIMATION: usize = 0x4000;
    const SEQUENCE_NAME: usize = 0x5000;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Call {
        IsSkeletal(usize),
        ActorAnimation(usize),
        DefaultAnimation(usize),
        Find(usize, usize, i32),
        SequenceGroup(usize),
    }

    #[derive(Default)]
    struct Context {
        skeletal: bool,
        actor_animation: usize,
        default_animation: usize,
        missing_sequence: bool,
        group: i32,
        calls: Vec<Call>,
        failure: Option<(usize, i32)>,
    }

    unsafe fn context<'a>(context: *mut c_void) -> &'a mut Context {
        unsafe { &mut *context.cast::<Context>() }
    }

    unsafe fn status(context: &mut Context) -> i32 {
        let index = context.calls.len() - 1;
        context
            .failure
            .filter(|(failed_at, _)| *failed_at == index)
            .map_or(0, |(_, status)| status)
    }

    unsafe extern "C" fn mesh_is_skeletal(
        context_ptr: *mut c_void,
        mesh: *mut c_void,
        out: *mut i32,
    ) -> i32 {
        let context = unsafe { context(context_ptr) };
        context.calls.push(Call::IsSkeletal(mesh as usize));
        unsafe { *out = i32::from(context.skeletal) };
        unsafe { status(context) }
    }

    unsafe extern "C" fn actor_skeletal_animation(
        context_ptr: *mut c_void,
        actor: *mut c_void,
        out: *mut *mut c_void,
    ) -> i32 {
        let context = unsafe { context(context_ptr) };
        context.calls.push(Call::ActorAnimation(actor as usize));
        unsafe { *out = context.actor_animation as *mut c_void };
        unsafe { status(context) }
    }

    unsafe extern "C" fn mesh_default_animation(
        context_ptr: *mut c_void,
        mesh: *mut c_void,
        out: *mut *mut c_void,
    ) -> i32 {
        let context = unsafe { context(context_ptr) };
        context.calls.push(Call::DefaultAnimation(mesh as usize));
        unsafe { *out = context.default_animation as *mut c_void };
        unsafe { status(context) }
    }

    unsafe extern "C" fn find_sequence(
        context_ptr: *mut c_void,
        owner: *mut c_void,
        name: *mut c_void,
        owner_kind: i32,
        out: *mut *mut c_void,
    ) -> i32 {
        let context = unsafe { context(context_ptr) };
        context
            .calls
            .push(Call::Find(owner as usize, name as usize, owner_kind));
        unsafe {
            *out = if context.missing_sequence {
                std::ptr::null_mut()
            } else {
                (owner as usize + 1) as *mut c_void
            }
        };
        unsafe { status(context) }
    }

    unsafe extern "C" fn sequence_group(
        context_ptr: *mut c_void,
        sequence: *mut c_void,
        out: *mut c_void,
    ) -> i32 {
        let context = unsafe { context(context_ptr) };
        context.calls.push(Call::SequenceGroup(sequence as usize));
        unsafe { out.cast::<i32>().write_unaligned(context.group) };
        unsafe { status(context) }
    }

    #[test]
    fn has_anim_reports_found_and_missing_sequences() {
        let mut context = Context::default();
        let services = services(&mut context);
        let mut out = -1;
        assert_eq!(
            unsafe {
                has_anim(
                    ACTOR as *mut c_void,
                    MESH as *mut c_void,
                    SEQUENCE_NAME as *mut c_void,
                    &mut out,
                    &services,
                )
            },
            0
        );
        assert_eq!(out, 1);

        context.calls.clear();
        context.missing_sequence = true;
        assert_eq!(
            unsafe {
                has_anim(
                    ACTOR as *mut c_void,
                    MESH as *mut c_void,
                    SEQUENCE_NAME as *mut c_void,
                    &mut out,
                    &services,
                )
            },
            0
        );
        assert_eq!(out, 0);
        assert_eq!(
            context.calls,
            [
                Call::IsSkeletal(MESH),
                Call::Find(MESH, SEQUENCE_NAME, OWNER_MESH)
            ]
        );
    }

    #[test]
    fn group_query_copies_group_and_reports_missing_sequence() {
        let mut context = Context {
            group: 0x1234,
            ..Context::default()
        };
        let animation_services = services(&mut context);
        let group_services = Hp2ActorAnimGroupServices {
            context: (&mut context as *mut Context).cast(),
            sequence_group,
        };
        let mut group = -1i32;
        let mut found = -1;
        assert_eq!(
            unsafe {
                get_anim_group(
                    ACTOR as *mut c_void,
                    MESH as *mut c_void,
                    SEQUENCE_NAME as *mut c_void,
                    (&mut group as *mut i32).cast(),
                    &mut found,
                    &animation_services,
                    &group_services,
                )
            },
            0
        );
        assert_eq!(group, 0x1234);
        assert_eq!(found, 1);
        assert_eq!(
            context.calls,
            [
                Call::IsSkeletal(MESH),
                Call::Find(MESH, SEQUENCE_NAME, OWNER_MESH),
                Call::SequenceGroup(MESH + 1)
            ]
        );

        context.calls.clear();
        context.missing_sequence = true;
        group = -1;
        found = -1;
        assert_eq!(
            unsafe {
                get_anim_group(
                    ACTOR as *mut c_void,
                    MESH as *mut c_void,
                    SEQUENCE_NAME as *mut c_void,
                    (&mut group as *mut i32).cast(),
                    &mut found,
                    &animation_services,
                    &group_services,
                )
            },
            0
        );
        assert_eq!(group, 0);
        assert_eq!(found, 0);
        assert_eq!(
            context.calls,
            [
                Call::IsSkeletal(MESH),
                Call::Find(MESH, SEQUENCE_NAME, OWNER_MESH)
            ]
        );
    }

    fn services(context: &mut Context) -> Hp2ActorAnimServices {
        Hp2ActorAnimServices {
            context: (context as *mut Context).cast(),
            mesh_is_skeletal,
            actor_skeletal_animation,
            mesh_default_animation,
            find_sequence,
        }
    }

    unsafe fn get(context: &mut Context) -> *mut c_void {
        let services = services(context);
        let mut out = std::ptr::null_mut();
        let status = unsafe {
            get_anim(
                ACTOR as *mut c_void,
                MESH as *mut c_void,
                SEQUENCE_NAME as *mut c_void,
                &mut out,
                &services,
            )
        };
        assert_eq!(status, 0);
        out
    }

    #[test]
    fn classic_mesh_looks_up_its_own_sequence_without_reading_actor_animation() {
        let mut context = Context::default();
        assert_eq!(unsafe { get(&mut context) }, (MESH + 1) as *mut c_void);
        assert_eq!(
            context.calls,
            [
                Call::IsSkeletal(MESH),
                Call::Find(MESH, SEQUENCE_NAME, OWNER_MESH)
            ]
        );
    }

    #[test]
    fn skeletal_mesh_prefers_actor_animation_over_default() {
        let mut context = Context {
            skeletal: true,
            actor_animation: SKELETAL_ANIMATION,
            ..Context::default()
        };
        assert_eq!(
            unsafe { get(&mut context) },
            (SKELETAL_ANIMATION + 1) as *mut c_void
        );
        assert_eq!(
            context.calls,
            [
                Call::IsSkeletal(MESH),
                Call::ActorAnimation(ACTOR),
                Call::Find(SKELETAL_ANIMATION, SEQUENCE_NAME, OWNER_SKELETAL_ANIMATION)
            ]
        );
    }

    #[test]
    fn skeletal_mesh_falls_back_to_default_animation_when_unlinked() {
        let mut context = Context {
            skeletal: true,
            default_animation: DEFAULT_ANIMATION,
            ..Context::default()
        };
        assert_eq!(
            unsafe { get(&mut context) },
            (DEFAULT_ANIMATION + 1) as *mut c_void
        );
        assert_eq!(
            context.calls,
            [
                Call::IsSkeletal(MESH),
                Call::ActorAnimation(ACTOR),
                Call::DefaultAnimation(MESH),
                Call::Find(DEFAULT_ANIMATION, SEQUENCE_NAME, OWNER_SKELETAL_ANIMATION)
            ]
        );
    }

    #[test]
    fn callback_error_stops_sequence_lookup() {
        let mut context = Context {
            skeletal: true,
            failure: Some((1, 23)),
            ..Context::default()
        };
        let services = services(&mut context);
        let mut out = std::ptr::null_mut();
        let status = unsafe {
            get_anim(
                ACTOR as *mut c_void,
                MESH as *mut c_void,
                SEQUENCE_NAME as *mut c_void,
                &mut out,
                &services,
            )
        };
        assert_eq!(status, 23);
        assert!(out.is_null());
        assert_eq!(
            context.calls,
            [Call::IsSkeletal(MESH), Call::ActorAnimation(ACTOR)]
        );
    }
}

#[cfg(test)]
mod channel_tests {
    use super::*;

    const ACTOR: usize = 0x1000;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Call {
        Count,
        Bone(i32),
        IsAnimating(i32),
    }

    #[derive(Default)]
    struct Context {
        channels: Vec<(i32, bool)>,
        calls: Vec<Call>,
        failure: Option<Call>,
    }

    unsafe fn context(context: *mut c_void) -> &'static mut Context {
        unsafe { &mut *context.cast::<Context>() }
    }

    unsafe fn status(context: &Context, call: Call) -> i32 {
        if context.failure == Some(call) { -7 } else { 0 }
    }

    unsafe extern "C" fn auxiliary_count(
        context_ptr: *mut c_void,
        _actor: *mut c_void,
        out: *mut i32,
    ) -> i32 {
        let context = unsafe { context(context_ptr) };
        context.calls.push(Call::Count);
        unsafe { *out = context.channels.len() as i32 };
        unsafe { status(context, Call::Count) }
    }

    unsafe extern "C" fn auxiliary_bone(
        context_ptr: *mut c_void,
        _actor: *mut c_void,
        index: i32,
        out: *mut i32,
    ) -> i32 {
        let context = unsafe { context(context_ptr) };
        context.calls.push(Call::Bone(index));
        unsafe { *out = context.channels[index as usize].0 };
        unsafe { status(context, Call::Bone(index)) }
    }

    unsafe extern "C" fn auxiliary_is_animating(
        context_ptr: *mut c_void,
        _actor: *mut c_void,
        index: i32,
        out: *mut i32,
    ) -> i32 {
        let context = unsafe { context(context_ptr) };
        context.calls.push(Call::IsAnimating(index));
        unsafe { *out = i32::from(context.channels[index as usize].1) };
        unsafe { status(context, Call::IsAnimating(index)) }
    }

    fn services(context: &mut Context) -> Hp2ActorAnimChannelServices {
        Hp2ActorAnimChannelServices {
            context: (context as *mut Context).cast(),
            auxiliary_count,
            auxiliary_bone,
            auxiliary_is_animating,
        }
    }

    unsafe fn query(
        actor: *mut c_void,
        bone_is_none: bool,
        mesh_is_skeletal: bool,
        bone_index: i32,
        context: &mut Context,
    ) -> (i32, i32) {
        let services = services(context);
        let mut out = -1;
        let status = unsafe {
            is_animating_channel(
                actor,
                i32::from(bone_is_none),
                i32::from(mesh_is_skeletal),
                bone_index,
                &mut out,
                &services,
            )
        };
        (status, out)
    }

    #[test]
    fn base_animation_uses_actor_predicate_without_channel_services() {
        let mut actor: hp2_abi::engine::generated::AActor = unsafe { std::mem::zeroed() };
        actor.anim_sequence = [1, 0, 0, 0];
        actor.anim_rate = 1.0;
        let mut context = Context::default();

        assert_eq!(
            unsafe {
                query(
                    (&mut actor as *mut hp2_abi::engine::generated::AActor).cast(),
                    true,
                    false,
                    0,
                    &mut context,
                )
            },
            (0, 1)
        );
        assert!(context.calls.is_empty());
    }

    #[test]
    fn nonskeletal_mesh_has_no_bone_animation_channel() {
        let mut context = Context::default();
        assert_eq!(
            unsafe { query(ACTOR as *mut c_void, false, false, 3, &mut context,) },
            (0, 0)
        );
        assert!(context.calls.is_empty());
    }

    #[test]
    fn first_matching_channel_wins_even_when_a_later_duplicate_is_animating() {
        let mut context = Context {
            channels: vec![(2, false), (3, false), (3, true)],
            ..Context::default()
        };
        assert_eq!(
            unsafe { query(ACTOR as *mut c_void, false, true, 3, &mut context,) },
            (0, 0)
        );
        assert_eq!(
            context.calls,
            [
                Call::Count,
                Call::Bone(0),
                Call::Bone(1),
                Call::IsAnimating(1)
            ]
        );
    }

    #[test]
    fn matching_channel_returns_its_animation_state() {
        let mut context = Context {
            channels: vec![(3, true)],
            ..Context::default()
        };
        assert_eq!(
            unsafe { query(ACTOR as *mut c_void, false, true, 3, &mut context,) },
            (0, 1)
        );
        assert_eq!(
            context.calls,
            [Call::Count, Call::Bone(0), Call::IsAnimating(0)]
        );
    }

    #[test]
    fn missing_bone_channel_returns_false() {
        let mut context = Context {
            channels: vec![(2, true), (4, true)],
            ..Context::default()
        };
        assert_eq!(
            unsafe { query(ACTOR as *mut c_void, false, true, 3, &mut context,) },
            (0, 0)
        );
        assert_eq!(context.calls, [Call::Count, Call::Bone(0), Call::Bone(1)]);
    }

    #[test]
    fn propagates_auxiliary_lookup_failures() {
        let mut context = Context {
            channels: vec![(3, true)],
            failure: Some(Call::Bone(0)),
            ..Context::default()
        };
        assert_eq!(
            unsafe { query(ACTOR as *mut c_void, false, true, 3, &mut context,) },
            (-7, 0)
        );
        assert_eq!(context.calls, [Call::Count, Call::Bone(0)]);
    }
}

#[cfg(test)]
mod bone_number_tests {
    use super::*;

    const MESH: usize = 0x6000;
    const BONE_NAME: usize = 0x7000;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Call {
        IsSkeletal(usize),
        BoneIndex(usize, usize),
    }

    struct Context {
        skeletal: bool,
        bone_index: i32,
        calls: Vec<Call>,
        failure: Option<(Call, i32)>,
    }

    unsafe fn context(context: *mut c_void) -> &'static mut Context {
        unsafe { &mut *context.cast::<Context>() }
    }

    unsafe fn status(context: &Context, call: Call) -> i32 {
        context
            .failure
            .filter(|(failed_call, _)| *failed_call == call)
            .map_or(0, |(_, status)| status)
    }

    unsafe extern "C" fn mesh_is_skeletal(
        context_ptr: *mut c_void,
        mesh: *mut c_void,
        out: *mut i32,
    ) -> i32 {
        let context = unsafe { context(context_ptr) };
        let call = Call::IsSkeletal(mesh as usize);
        context.calls.push(call);
        unsafe { *out = i32::from(context.skeletal) };
        unsafe { status(context, call) }
    }

    unsafe extern "C" fn bone_index(
        context_ptr: *mut c_void,
        mesh: *mut c_void,
        name: *mut c_void,
        out: *mut i32,
    ) -> i32 {
        let context = unsafe { context(context_ptr) };
        let call = Call::BoneIndex(mesh as usize, name as usize);
        context.calls.push(call);
        unsafe { *out = context.bone_index };
        unsafe { status(context, call) }
    }

    fn services(context: &mut Context) -> Hp2ActorBoneNumberServices {
        Hp2ActorBoneNumberServices {
            context: (context as *mut Context).cast(),
            mesh_is_skeletal,
            bone_index,
        }
    }

    unsafe fn query(mesh: *mut c_void, context: &mut Context) -> (i32, i32) {
        let services = services(context);
        let mut out = -1;
        let status = unsafe { bone_number(mesh, BONE_NAME as *mut c_void, &mut out, &services) };
        (status, out)
    }

    #[test]
    fn nonskeletal_mesh_returns_zero_without_calling_bone_index() {
        let mut context = Context {
            skeletal: false,
            bone_index: 8,
            calls: Vec::new(),
            failure: None,
        };
        assert_eq!(unsafe { query(MESH as *mut c_void, &mut context) }, (0, 0));
        assert_eq!(context.calls, [Call::IsSkeletal(MESH)]);
    }

    #[test]
    fn skeletal_mesh_returns_the_virtual_bone_index() {
        let mut context = Context {
            skeletal: true,
            bone_index: 8,
            calls: Vec::new(),
            failure: None,
        };
        assert_eq!(unsafe { query(MESH as *mut c_void, &mut context) }, (0, 8));
        assert_eq!(
            context.calls,
            [Call::IsSkeletal(MESH), Call::BoneIndex(MESH, BONE_NAME)]
        );
    }

    #[test]
    fn null_mesh_returns_zero_without_callbacks() {
        let mut context = Context {
            skeletal: true,
            bone_index: 8,
            calls: Vec::new(),
            failure: None,
        };
        assert_eq!(unsafe { query(std::ptr::null_mut(), &mut context) }, (0, 0));
        assert!(context.calls.is_empty());
    }

    #[test]
    fn callback_errors_propagate_and_stop_the_lookup() {
        let fail_at = Call::IsSkeletal(MESH);
        let mut context = Context {
            skeletal: true,
            bone_index: 8,
            calls: Vec::new(),
            failure: Some((fail_at, -5)),
        };
        assert_eq!(unsafe { query(MESH as *mut c_void, &mut context) }, (-5, 0));
        assert_eq!(context.calls, [fail_at]);
    }
}

#[cfg(test)]
mod bone_name_tests {
    use super::*;
    use hp2_abi::engine::generated::FName;

    const MESH: usize = 0x8000;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Call {
        IsSkeletal(usize),
        BoneName(usize, i32),
    }

    struct Context {
        skeletal: bool,
        bone_name: i32,
        calls: Vec<Call>,
        failure: Option<(Call, i32)>,
    }

    unsafe fn context(context: *mut c_void) -> &'static mut Context {
        unsafe { &mut *context.cast::<Context>() }
    }

    unsafe fn status(context: &Context, call: Call) -> i32 {
        context
            .failure
            .filter(|(failed_call, _)| *failed_call == call)
            .map_or(0, |(_, status)| status)
    }

    unsafe extern "C" fn mesh_is_skeletal(
        context_ptr: *mut c_void,
        mesh: *mut c_void,
        out: *mut i32,
    ) -> i32 {
        let context = unsafe { context(context_ptr) };
        let call = Call::IsSkeletal(mesh as usize);
        context.calls.push(call);
        unsafe { *out = i32::from(context.skeletal) };
        unsafe { status(context, call) }
    }

    unsafe extern "C" fn bone_name_callback(
        context_ptr: *mut c_void,
        mesh: *mut c_void,
        bone_number: i32,
        out: *mut c_void,
    ) -> i32 {
        let context = unsafe { context(context_ptr) };
        let call = Call::BoneName(mesh as usize, bone_number);
        context.calls.push(call);
        unsafe { (*out.cast::<FName>()).index = context.bone_name };
        unsafe { status(context, call) }
    }

    fn services(context: &mut Context) -> Hp2ActorBoneNameServices {
        Hp2ActorBoneNameServices {
            context: (context as *mut Context).cast(),
            mesh_is_skeletal,
            bone_name: bone_name_callback,
        }
    }

    unsafe fn query(mesh: *mut c_void, bone_number: i32, context: &mut Context) -> (i32, FName) {
        let services = services(context);
        let mut out = FName { index: -1 };
        let status = unsafe {
            super::bone_name(
                mesh,
                bone_number,
                (&mut out as *mut FName).cast(),
                &services,
            )
        };
        (status, out)
    }

    #[test]
    fn nonskeletal_mesh_returns_name_none_without_bone_lookup() {
        let mut context = Context {
            skeletal: false,
            bone_name: 24,
            calls: Vec::new(),
            failure: None,
        };
        let (status, out) = unsafe { query(MESH as *mut c_void, 3, &mut context) };
        assert_eq!(status, 0);
        assert_eq!(out.index, 0);
        assert_eq!(context.calls, [Call::IsSkeletal(MESH)]);
    }

    #[test]
    fn skeletal_mesh_returns_name_for_requested_bone_number() {
        let mut context = Context {
            skeletal: true,
            bone_name: 24,
            calls: Vec::new(),
            failure: None,
        };
        let (status, out) = unsafe { query(MESH as *mut c_void, 3, &mut context) };
        assert_eq!(status, 0);
        assert_eq!(out.index, 24);
        assert_eq!(
            context.calls,
            [Call::IsSkeletal(MESH), Call::BoneName(MESH, 3)]
        );
    }

    #[test]
    fn null_mesh_returns_name_none_without_callbacks() {
        let mut context = Context {
            skeletal: true,
            bone_name: 24,
            calls: Vec::new(),
            failure: None,
        };
        let (status, out) = unsafe { query(std::ptr::null_mut(), 3, &mut context) };
        assert_eq!(status, 0);
        assert_eq!(out.index, 0);
        assert!(context.calls.is_empty());
    }

    #[test]
    fn mesh_classification_error_stops_lookup_and_preserves_none() {
        let failed = Call::IsSkeletal(MESH);
        let mut context = Context {
            skeletal: true,
            bone_name: 24,
            calls: Vec::new(),
            failure: Some((failed, -9)),
        };
        let (status, out) = unsafe { query(MESH as *mut c_void, 3, &mut context) };
        assert_eq!(status, -9);
        assert_eq!(out.index, 0);
        assert_eq!(context.calls, [failed]);
    }
}

#[cfg(test)]
mod bone_transform_tests {
    use super::*;
    use hp2_abi::FVector;
    use hp2_abi::engine::generated::{AActor, FRotator};

    const MESH: usize = 0xA000;
    const BONE_NAME: usize = 0xB000;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Call {
        IsSkeletal,
        BoneIndex,
        Position(i32),
        Rotation(i32),
    }

    struct Context {
        skeletal: bool,
        bone_index: i32,
        position: FVector,
        rotation: FRotator,
        calls: Vec<Call>,
        failure: Option<(Call, i32)>,
    }

    unsafe fn context(context: *mut c_void) -> &'static mut Context {
        unsafe { &mut *context.cast::<Context>() }
    }

    unsafe fn status(context: &Context, call: Call) -> i32 {
        context
            .failure
            .filter(|(failed_call, _)| *failed_call == call)
            .map_or(0, |(_, status)| status)
    }

    unsafe extern "C" fn mesh_is_skeletal(
        context_ptr: *mut c_void,
        _mesh: *mut c_void,
        out: *mut i32,
    ) -> i32 {
        let context = unsafe { context(context_ptr) };
        context.calls.push(Call::IsSkeletal);
        unsafe { *out = i32::from(context.skeletal) };
        unsafe { status(context, Call::IsSkeletal) }
    }

    unsafe extern "C" fn bone_index(
        context_ptr: *mut c_void,
        _mesh: *mut c_void,
        _bone_name: *mut c_void,
        out: *mut i32,
    ) -> i32 {
        let context = unsafe { context(context_ptr) };
        context.calls.push(Call::BoneIndex);
        unsafe { *out = context.bone_index };
        unsafe { status(context, Call::BoneIndex) }
    }

    unsafe extern "C" fn bone_position(
        context_ptr: *mut c_void,
        _actor: *mut c_void,
        _mesh: *mut c_void,
        index: i32,
        out: *mut c_void,
    ) -> i32 {
        let context = unsafe { context(context_ptr) };
        let call = Call::Position(index);
        context.calls.push(call);
        unsafe { *out.cast::<FVector>() = context.position };
        unsafe { status(context, call) }
    }

    unsafe extern "C" fn bone_rotation(
        context_ptr: *mut c_void,
        _actor: *mut c_void,
        _mesh: *mut c_void,
        index: i32,
        out: *mut c_void,
    ) -> i32 {
        let context = unsafe { context(context_ptr) };
        let call = Call::Rotation(index);
        context.calls.push(call);
        unsafe { *out.cast::<FRotator>() = context.rotation };
        unsafe { status(context, call) }
    }

    fn services(context: &mut Context) -> Hp2ActorBoneTransformServices {
        Hp2ActorBoneTransformServices {
            context: (context as *mut Context).cast(),
            mesh_is_skeletal,
            bone_index,
            bone_position,
            bone_rotation,
        }
    }

    fn actor() -> AActor {
        let mut actor: AActor = unsafe { std::mem::zeroed() };
        actor.location = FVector {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        actor.rotation = FRotator {
            pitch: 10,
            yaw: 20,
            roll: 30,
        };
        actor
    }

    unsafe fn query_position(
        actor: &mut AActor,
        mesh: *mut c_void,
        context: &mut Context,
    ) -> (i32, FVector) {
        let services = services(context);
        let mut out = FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let status = unsafe {
            bone_transform(
                (actor as *mut AActor).cast(),
                mesh,
                BONE_NAME as *mut c_void,
                BONE_TRANSFORM_POSITION,
                (&mut out as *mut FVector).cast(),
                &services,
            )
        };
        (status, out)
    }

    unsafe fn query_rotation(
        actor: &mut AActor,
        mesh: *mut c_void,
        context: &mut Context,
    ) -> (i32, FRotator) {
        let services = services(context);
        let mut out = FRotator {
            pitch: -1,
            yaw: -1,
            roll: -1,
        };
        let status = unsafe {
            bone_transform(
                (actor as *mut AActor).cast(),
                mesh,
                BONE_NAME as *mut c_void,
                BONE_TRANSFORM_ROTATION,
                (&mut out as *mut FRotator).cast(),
                &services,
            )
        };
        (status, out)
    }

    #[test]
    fn nonskeletal_mesh_preserves_actor_position_and_rotation() {
        let mut actor = actor();
        let mut context = Context {
            skeletal: false,
            bone_index: 4,
            position: FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            rotation: FRotator {
                pitch: 0,
                yaw: 0,
                roll: 0,
            },
            calls: Vec::new(),
            failure: None,
        };
        assert_eq!(
            unsafe { query_position(&mut actor, MESH as *mut c_void, &mut context) },
            (0, actor.location)
        );
        let (status, actual) =
            unsafe { query_rotation(&mut actor, MESH as *mut c_void, &mut context) };
        assert_eq!(status, 0);
        assert_eq!(
            (actual.pitch, actual.yaw, actual.roll),
            (
                actor.rotation.pitch,
                actor.rotation.yaw,
                actor.rotation.roll
            )
        );
        assert_eq!(context.calls, [Call::IsSkeletal, Call::IsSkeletal]);
    }

    #[test]
    fn skeletal_position_uses_the_selected_bone_transform() {
        let mut actor = actor();
        let position = FVector {
            x: -4.0,
            y: 5.0,
            z: 6.0,
        };
        let mut context = Context {
            skeletal: true,
            bone_index: 7,
            position,
            rotation: FRotator {
                pitch: 0,
                yaw: 0,
                roll: 0,
            },
            calls: Vec::new(),
            failure: None,
        };
        assert_eq!(
            unsafe { query_position(&mut actor, MESH as *mut c_void, &mut context) },
            (0, position)
        );
        assert_eq!(
            context.calls,
            [Call::IsSkeletal, Call::BoneIndex, Call::Position(7)]
        );
    }

    #[test]
    fn skeletal_rotation_uses_the_selected_bone_transform() {
        let mut actor = actor();
        let rotation = FRotator {
            pitch: -120,
            yaw: 400,
            roll: 0,
        };
        let mut context = Context {
            skeletal: true,
            bone_index: 7,
            position: FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            rotation,
            calls: Vec::new(),
            failure: None,
        };
        let (status, actual) =
            unsafe { query_rotation(&mut actor, MESH as *mut c_void, &mut context) };
        assert_eq!(status, 0);
        assert_eq!(
            (actual.pitch, actual.yaw, actual.roll),
            (rotation.pitch, rotation.yaw, rotation.roll)
        );
        assert_eq!(
            context.calls,
            [Call::IsSkeletal, Call::BoneIndex, Call::Rotation(7)]
        );
    }

    #[test]
    fn missing_mesh_returns_actor_position_without_callbacks() {
        let mut actor = actor();
        let mut context = Context {
            skeletal: true,
            bone_index: 7,
            position: FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            rotation: FRotator {
                pitch: 0,
                yaw: 0,
                roll: 0,
            },
            calls: Vec::new(),
            failure: None,
        };
        assert_eq!(
            unsafe { query_position(&mut actor, std::ptr::null_mut(), &mut context) },
            (0, actor.location)
        );
        assert!(context.calls.is_empty());
    }

    #[test]
    fn index_callback_error_stops_transform_lookup() {
        let mut actor = actor();
        let mut context = Context {
            skeletal: true,
            bone_index: 7,
            position: FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            rotation: FRotator {
                pitch: 0,
                yaw: 0,
                roll: 0,
            },
            calls: Vec::new(),
            failure: Some((Call::BoneIndex, -3)),
        };
        assert_eq!(
            unsafe { query_position(&mut actor, MESH as *mut c_void, &mut context) },
            (-3, actor.location)
        );
        assert_eq!(context.calls, [Call::IsSkeletal, Call::BoneIndex]);
    }
}
#[cfg(test)]
mod link_skeletal_animation_tests {
    use super::*;
    use hp2_abi::engine::generated::AActor;

    #[test]
    fn assigns_and_clears_the_linked_animation() {
        let mut actor: AActor = unsafe { std::mem::zeroed() };
        let animation = 0xA000usize as *mut c_void;
        assert_eq!(
            unsafe { link_skeletal_animation((&mut actor as *mut AActor).cast(), animation) },
            0
        );
        let actual = actor.skel_anim;
        assert_eq!(actual, animation);

        assert_eq!(
            unsafe {
                link_skeletal_animation((&mut actor as *mut AActor).cast(), std::ptr::null_mut())
            },
            0
        );
        let actual = actor.skel_anim;
        assert!(actual.is_null());
    }

    #[test]
    fn null_actor_is_rejected() {
        assert_eq!(
            unsafe { link_skeletal_animation(std::ptr::null_mut(), std::ptr::null_mut()) },
            1
        );
    }
}

/*-----------------------------------------------------------------------------
    Publish-once storage for UnScript.cpp's animation service tables. Each
    table is a C++ file-scope `static const`, published exactly once via a
    file-scope static initializer (mirrors `hp2_core::vm_object::publish_services`).
-----------------------------------------------------------------------------*/

static mut ANIM_SERVICES: Option<Hp2ActorAnimServices> = None;

/// # Safety
/// `value` is the real, immutable `GActorAnimServices` table; publication
/// happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_anim_services(value: Hp2ActorAnimServices) {
    unsafe { (&raw mut ANIM_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_anim_services` has run.
pub unsafe fn published_anim_services() -> Result<Hp2ActorAnimServices, i32> {
    unsafe { (&raw const ANIM_SERVICES).read().ok_or(1) }
}

static mut ANIM_GROUP_SERVICES: Option<Hp2ActorAnimGroupServices> = None;

/// # Safety
/// `value` is the real, immutable `GActorAnimGroupServices` table; publication
/// happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_anim_group_services(value: Hp2ActorAnimGroupServices) {
    unsafe { (&raw mut ANIM_GROUP_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_anim_group_services` has run.
pub unsafe fn published_anim_group_services() -> Result<Hp2ActorAnimGroupServices, i32> {
    unsafe { (&raw const ANIM_GROUP_SERVICES).read().ok_or(1) }
}

static mut ANIM_CHANNEL_SERVICES: Option<Hp2ActorAnimChannelServices> = None;

/// # Safety
/// `value` is the real, immutable `GActorAnimChannelServices` table; publication
/// happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_anim_channel_services(value: Hp2ActorAnimChannelServices) {
    unsafe { (&raw mut ANIM_CHANNEL_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_anim_channel_services` has run.
pub unsafe fn published_anim_channel_services() -> Result<Hp2ActorAnimChannelServices, i32> {
    unsafe { (&raw const ANIM_CHANNEL_SERVICES).read().ok_or(1) }
}

static mut BONE_NUMBER_SERVICES: Option<Hp2ActorBoneNumberServices> = None;

/// # Safety
/// `value` is the real, immutable `GActorBoneNumberServices` table; publication
/// happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_bone_number_services(value: Hp2ActorBoneNumberServices) {
    unsafe { (&raw mut BONE_NUMBER_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_bone_number_services` has run.
pub unsafe fn published_bone_number_services() -> Result<Hp2ActorBoneNumberServices, i32> {
    unsafe { (&raw const BONE_NUMBER_SERVICES).read().ok_or(1) }
}

static mut BONE_NAME_SERVICES: Option<Hp2ActorBoneNameServices> = None;

/// # Safety
/// `value` is the real, immutable `GActorBoneNameServices` table; publication
/// happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_bone_name_services(value: Hp2ActorBoneNameServices) {
    unsafe { (&raw mut BONE_NAME_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_bone_name_services` has run.
pub unsafe fn published_bone_name_services() -> Result<Hp2ActorBoneNameServices, i32> {
    unsafe { (&raw const BONE_NAME_SERVICES).read().ok_or(1) }
}

static mut BONE_TRANSFORM_SERVICES: Option<Hp2ActorBoneTransformServices> = None;

/// # Safety
/// `value` is the real, immutable `GActorBoneTransformServices` table;
/// publication happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_bone_transform_services(value: Hp2ActorBoneTransformServices) {
    unsafe { (&raw mut BONE_TRANSFORM_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_bone_transform_services` has run.
pub unsafe fn published_bone_transform_services() -> Result<Hp2ActorBoneTransformServices, i32> {
    unsafe { (&raw const BONE_TRANSFORM_SERVICES).read().ok_or(1) }
}

static mut PLAY_ANIM_SERVICES: Option<Hp2ActorPlayAnimServices> = None;

/// # Safety
/// `value` is the real, immutable `GActorPlayAnimServices` table (its nested
/// `animation` pointer addresses the equally static `GActorAnimServices`);
/// publication happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_play_anim_services(value: Hp2ActorPlayAnimServices) {
    unsafe { (&raw mut PLAY_ANIM_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_play_anim_services` has run.
pub unsafe fn published_play_anim_services() -> Result<Hp2ActorPlayAnimServices, i32> {
    unsafe { (&raw const PLAY_ANIM_SERVICES).read().ok_or(1) }
}

static mut CREATE_ANIM_CHANNEL_SERVICES: Option<Hp2ActorCreateAnimChannelServices> = None;

/// # Safety
/// `value` is the real, immutable `GActorCreateAnimChannelServices` table;
/// publication happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_create_anim_channel_services(value: Hp2ActorCreateAnimChannelServices) {
    unsafe { (&raw mut CREATE_ANIM_CHANNEL_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_create_anim_channel_services` has run.
pub unsafe fn published_create_anim_channel_services()
-> Result<Hp2ActorCreateAnimChannelServices, i32> {
    unsafe { (&raw const CREATE_ANIM_CHANNEL_SERVICES).read().ok_or(1) }
}
