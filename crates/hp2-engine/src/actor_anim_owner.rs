//! Canonical Core-backed actor animation, channels, skeletal queries and lip sync.

use crate::actor_anim;
use hp2_abi::engine::generated::{AActor, FMeshAnimSeq, FName, UAnimation, UClass, UObject, USkeletalMesh, ULevel};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use std::{ffi::c_void, mem::size_of, ptr};

const ARRAY_TAG: &[u8] = b"AActor.AuxAnims\0";
const PLAY_ERROR: i32 = 1;

struct AnimContext<'a> {
    access: CoreAccess<'a>,
    channel_class: *mut UClass,
    error: Option<RuntimeError>,
}

impl AnimContext<'_> {
    fn remember(&mut self, result: RuntimeResult<()>) -> i32 {
        match result { Ok(()) => 0, Err(error) => { self.error = Some(error); PLAY_ERROR } }
    }
}

fn take_error(context: &mut AnimContext<'_>, status: i32) -> RuntimeResult<()> {
    if status == 0 { Ok(()) } else { Err(context.error.take().unwrap_or_else(|| RuntimeError::new("Canonical animation operation failed"))) }
}

unsafe extern "C" fn mesh_is_skeletal(ctx: *mut c_void, mesh: *mut c_void, out: *mut i32) -> i32 {
    if ctx.is_null() || mesh.is_null() || out.is_null() { return PLAY_ERROR; }
    let context = unsafe { &mut *ctx.cast::<AnimContext<'_>>() };
    let result = (|| {
        let class = context.access.native_class("Engine.SkeletalMesh")?;
        let actual = unsafe { ptr::addr_of!((*mesh.cast::<UObject>()).class).read_unaligned() };
        unsafe { out.write(i32::from(hp2_core::object_model::is_child_of(actual.cast(), class.cast()))) };
        Ok(())
    })();
    context.remember(result)
}

unsafe extern "C" fn bone_index(ctx: *mut c_void, mesh: *mut c_void, bone: *mut c_void, out: *mut i32) -> i32 {
    if ctx.is_null() || mesh.is_null() || bone.is_null() || out.is_null() { return PLAY_ERROR; }
    let _context = unsafe { &mut *ctx.cast::<AnimContext<'_>>() };
    let name = unsafe { bone.cast::<[u8; 4]>().read_unaligned() };
    unsafe { out.write(crate::skeletal::bone_index(mesh.cast::<USkeletalMesh>(), name)) };
    0
}

unsafe extern "C" fn root_is_move(ctx: *mut c_void, root: *mut c_void, out: *mut i32) -> i32 {
    if ctx.is_null() || root.is_null() || out.is_null() { return PLAY_ERROR; }
    let context = unsafe { &mut *ctx.cast::<AnimContext<'_>>() };
    let name = unsafe { root.cast::<FName>().read_unaligned() };
    let result = (|| {
        let is_move = name.index != 0
            && context.access.intern_str("Move", hp2_core::name_owner::FindName::Find)? == name.index;
        unsafe { out.write(i32::from(is_move)); }
        Ok(())
    })();
    context.remember(result)
}

unsafe extern "C" fn channel_not_replaceable(_ctx: *mut c_void, channel: *mut c_void, out: *mut i32) -> i32 {
    if channel.is_null() || out.is_null() { return PLAY_ERROR; }
    let channel = unsafe { &*channel.cast::<AActor>() };
    unsafe { out.write(i32::from(channel.b_anim_not_replaceable())) };
    0
}

unsafe extern "C" fn root_channel_class(ctx: *mut c_void) -> i32 {
    if ctx.is_null() { return PLAY_ERROR; }
    let context = unsafe { &mut *ctx.cast::<AnimContext<'_>>() };
    let result = (|| {
        let base = context.access.native_class("Engine.Actor")?;
        let outer = unsafe { (*base.cast::<UObject>()).outer.cast::<UObject>() };
        let name = "AnimChannel".encode_utf16().chain([0]).map(u32::from).collect::<Vec<_>>();
        context.channel_class = unsafe { context.access.static_load_class(base, outer, &name, 0)? };
        Ok(())
    })();
    context.remember(result)
}

unsafe extern "C" fn spawn_channel(ctx: *mut c_void, actor: *mut c_void, class: *mut c_void, bone: i32, transient: i32, protected: i32, out: *mut *mut c_void) -> i32 {
    if ctx.is_null() || actor.is_null() || class.is_null() || out.is_null() { return PLAY_ERROR; }
    let context = unsafe { &mut *ctx.cast::<AnimContext<'_>>() };
    let result = (|| {
        let actor = actor.cast::<AActor>();
        let level = unsafe { (*actor).x_level.cast::<ULevel>() };
        let (location, rotation) = unsafe { ((*actor).location, (*actor).rotation) };
        let channel = unsafe { crate::level_spawn_actor_owner::spawn_actor(context.access, level, class.cast::<UClass>(), FName { index: 0 }, actor, ptr::null_mut(), location, rotation, ptr::null_mut(), true, false)? };
        unsafe {
            if !channel.is_null() {
                (*channel).mesh = (*actor).mesh;
                (*channel).skel_anim = (*actor).skel_anim;
                (*channel).anim_bone = bone as u8;
                (*channel).set_b_anim_transient(transient != 0);
                (*channel).set_b_anim_not_replaceable(protected != 0);
            }
            out.write(channel.cast());
        }
        Ok(())
    })();
    context.remember(result)
}

unsafe fn actor_array_slots(actor: *mut AActor) -> hp2_core::array_storage::Hp2ArraySlots {
    unsafe { hp2_core::array_storage::Hp2ArraySlots { data: ptr::addr_of_mut!((*actor).aux_anims.data), num: ptr::addr_of_mut!((*actor).aux_anims.num), max: ptr::addr_of_mut!((*actor).aux_anims.max) } }
}

unsafe extern "C" fn insert_channel(ctx: *mut c_void, actor: *mut c_void, channel: *mut c_void) -> i32 {
    if ctx.is_null() || actor.is_null() || channel.is_null() { return PLAY_ERROR; }
    let context = unsafe { &mut *ctx.cast::<AnimContext<'_>>() };
    let slots = unsafe { actor_array_slots(actor.cast()) };
    context.remember(unsafe { hp2_core::array_storage::insert(slots, 0, 1, size_of::<*mut AActor>() as i32, ARRAY_TAG.as_ptr().cast(), hp2_core::array_storage::system_allocator()) }.map_err(|_| RuntimeError::new("Unable to insert auxiliary animation channel")).map(|_| unsafe { (*actor.cast::<AActor>()).aux_anims.data.cast::<*mut AActor>().write(channel.cast()) }))
}

unsafe extern "C" fn append_channel(ctx: *mut c_void, actor: *mut c_void, channel: *mut c_void) -> i32 {
    if ctx.is_null() || actor.is_null() || channel.is_null() { return PLAY_ERROR; }
    let context = unsafe { &mut *ctx.cast::<AnimContext<'_>>() };
    let actor = actor.cast::<AActor>();
    let slots = unsafe { actor_array_slots(actor) };
    let index = unsafe { (*actor).aux_anims.num };
    context.remember(unsafe { hp2_core::array_storage::insert(slots, index, 1, size_of::<*mut AActor>() as i32, ARRAY_TAG.as_ptr().cast(), hp2_core::array_storage::system_allocator()) }.map_err(|_| RuntimeError::new("Unable to append auxiliary animation channel")).map(|_| unsafe { (*actor).aux_anims.data.cast::<*mut AActor>().add(index as usize).write(channel.cast()) }))
}

unsafe extern "C" fn destroy_channel(ctx: *mut c_void, actor: *mut c_void, index: i32) -> i32 {
    if ctx.is_null() || actor.is_null() { return PLAY_ERROR; }
    let context = unsafe { &mut *ctx.cast::<AnimContext<'_>>() };
    let actor = actor.cast::<AActor>();
    let result = (|| {
        let array = unsafe { ptr::addr_of!((*actor).aux_anims).read_unaligned() };
        if index < 0 || index >= array.num || array.data.is_null() { return Err(RuntimeError::new("Invalid auxiliary animation index")); }
        let channel = unsafe { array.data.cast::<*mut AActor>().add(index as usize).read_unaligned() };
        let level = unsafe { (*actor).x_level.cast::<ULevel>() };
        unsafe { crate::level_lifecycle_owner::destroy_actor(context.access, level, channel, false)?; }
        unsafe { hp2_core::array_storage::remove(actor_array_slots(actor), index, 1, size_of::<*mut AActor>() as i32, ARRAY_TAG.as_ptr().cast(), hp2_core::array_storage::system_allocator()) }.map_err(|_| RuntimeError::new("Unable to remove auxiliary animation channel"))
    })();
    context.remember(result)
}

unsafe extern "C" fn spawn_play_channel(ctx: *mut c_void, actor: *mut c_void, animation_type: i32, root: *mut c_void, out: *mut *mut c_void) -> i32 {
    if ctx.is_null() || actor.is_null() || root.is_null() || out.is_null() { return PLAY_ERROR; }
    let context = unsafe { &mut *ctx.cast::<AnimContext<'_>>() };
    if context.channel_class.is_null() { return PLAY_ERROR; }
    let services = actor_anim::Hp2ActorCreateAnimChannelServices {
        context: ctx, mesh_is_skeletal, bone_index, channel_not_replaceable,
        spawn_channel, destroy_channel, insert_channel, append_channel,
    };
    unsafe { actor_anim::create_anim_channel(actor, context.channel_class.cast(), animation_type, root, 1, 0, out, &services) }
}

unsafe extern "C" fn find_sequence(ctx: *mut c_void, owner: *mut c_void, name: *mut c_void, kind: i32, out: *mut *mut c_void) -> i32 {
    if ctx.is_null() || owner.is_null() || name.is_null() || out.is_null() { return PLAY_ERROR; }
    let _context = unsafe { &mut *ctx.cast::<AnimContext<'_>>() };
    unsafe { out.write(ptr::null_mut()) };
    let name = unsafe { name.cast::<[u8; 4]>().read_unaligned() };
    let sequences = if kind == 0 {
        unsafe { ptr::addr_of!((*owner.cast::<hp2_abi::engine::generated::UMesh>()).anim_seqs).read_unaligned() }
    } else {
        unsafe { ptr::addr_of!((*owner.cast::<UAnimation>()).anim_seqs).read_unaligned() }
    };
    if sequences.num < 0 || sequences.num > sequences.max || (sequences.num > 0 && sequences.data.is_null()) { return PLAY_ERROR; }
    for index in 0..sequences.num as usize {
        let sequence = unsafe { &*sequences.data.cast::<FMeshAnimSeq>().add(index) };
        if sequence.name == name { unsafe { out.write((sequence as *const FMeshAnimSeq).cast_mut().cast()) }; return 0; }
    }
    0
}

pub(crate) fn lipsync_start(access: CoreAccess<'_>, actor: *mut AActor) -> RuntimeResult<*mut AActor> {
    if actor.is_null() { return Err(RuntimeError::new("Null actor for lip-sync channel")); }
    let mut context = AnimContext { access, channel_class: ptr::null_mut(), error: None };
    let base = access.native_class("Engine.Actor")?;
    let outer = unsafe { (*base.cast::<UObject>()).outer.cast::<UObject>() };
    let name = "AnimChannel".encode_utf16().chain([0]).map(u32::from).collect::<Vec<_>>();
    let class = unsafe { access.static_load_class(base, outer, &name, 0)? };
    context.channel_class = class;
    let root = access.intern_str("mouth_root", hp2_core::name_owner::FindName::Add)?;
    let mut out = ptr::null_mut();
    let services = actor_anim::Hp2ActorCreateAnimChannelServices { context: (&mut context as *mut AnimContext<'_>).cast(), mesh_is_skeletal, bone_index, channel_not_replaceable, spawn_channel, destroy_channel, insert_channel, append_channel };
    let status = unsafe { actor_anim::create_anim_channel(actor.cast(), class.cast(), 0, (&root as *const _ as *mut c_void), 0, 1, &mut out, &services) };
    take_error(&mut context, status)?;
    if out.is_null() { return Ok(ptr::null_mut()); }
    let sequence = access.intern_str("mouth_anim", hp2_core::name_owner::FindName::Add)?;
    play(access, out.cast(), FName { index: sequence }, 0, 0.0, 0.0, 0.0, 0, FName { index: 0 })?;
    Ok(out.cast())
}

pub(crate) fn lipsync_release(access: CoreAccess<'_>, channel: *mut AActor) -> RuntimeResult<()> {
    if channel.is_null() { return Err(RuntimeError::new("Null lip-sync animation channel")); }
    let rest = access.intern_str("mouth_rest", hp2_core::name_owner::FindName::Add)?;
    play(access, channel, FName { index: rest }, 0, 1.0, 1.0, 1.0, 0, FName { index: 0 }).map(|_| ())
}

pub(crate) fn play(access: CoreAccess<'_>, actor: *mut AActor, sequence: FName, looping: i32, rate: f32, tween: f32, min_rate: f32, animation_type: i32, root: FName) -> RuntimeResult<i32> {
    let mut context = AnimContext { access, channel_class: ptr::null_mut(), error: None };
    let anim_services = actor_anim::Hp2ActorAnimServices { context: (&mut context as *mut AnimContext<'_>).cast(), mesh_is_skeletal, actor_skeletal_animation: linked_animation, mesh_default_animation: default_animation, find_sequence };
    let services = actor_anim::Hp2ActorPlayAnimServices { context: (&mut context as *mut AnimContext<'_>).cast(), animation: &anim_services, root_is_move, bone_index, channel_not_replaceable, load_channel_class: root_channel_class, create_channel: spawn_play_channel, destroy_channel, play_channel: play_subchannel, log: log_anim };
    let mut result = 0;
    let status = unsafe { actor_anim::play_anim(actor.cast(), (&sequence as *const _ as *mut c_void), looping, rate, tween, min_rate, animation_type, (&root as *const _ as *mut c_void), &mut result, &services) };
    take_error(&mut context, status)?;
    Ok(result)
}

unsafe extern "C" fn linked_animation(_ctx: *mut c_void, actor: *mut c_void, out: *mut *mut c_void) -> i32 { if actor.is_null() || out.is_null() { return PLAY_ERROR; } unsafe { out.write((*actor.cast::<AActor>()).skel_anim) }; 0 }
unsafe extern "C" fn default_animation(_ctx: *mut c_void, mesh: *mut c_void, out: *mut *mut c_void) -> i32 { if mesh.is_null() || out.is_null() { return PLAY_ERROR; } unsafe { out.write((*mesh.cast::<USkeletalMesh>()).default_animation) }; 0 }
unsafe extern "C" fn play_subchannel(ctx: *mut c_void, channel: *mut c_void, sequence: *mut c_void, looping: i32, rate: f32, tween: f32, min_rate: f32, out: *mut i32) -> i32 {
    if ctx.is_null() || channel.is_null() || sequence.is_null() || out.is_null() { return PLAY_ERROR; }
    let context = unsafe { &mut *ctx.cast::<AnimContext<'_>>() };
    let sequence = unsafe { sequence.cast::<FName>().read_unaligned() };
    let root = FName { index: 0 };
    match play(context.access, channel.cast(), sequence, looping, rate, tween, min_rate, 0, root) {
        Ok(value) => { unsafe { out.write(value) }; 0 }
        Err(error) => { context.error = Some(error); PLAY_ERROR }
    }
}
unsafe extern "C" fn log_anim(ctx: *mut c_void, actor: *mut c_void, name: *mut c_void, missing_mesh: i32) -> i32 {
    if ctx.is_null() || name.is_null() || actor.is_null() { return PLAY_ERROR; }
    let context = unsafe { &mut *ctx.cast::<AnimContext<'_>>() };
    let result = (|| {
        let category = context.access.intern_str("ScriptWarning", hp2_core::name_owner::FindName::Add)?;
        let sequence = unsafe { name.cast::<FName>().read_unaligned() };
        let label = context.access.name_text(sequence.index)?;
        let actor_name = context.access.name_text(i32::from_ne_bytes(unsafe { (*actor.cast::<UObject>()).name }))?;
        let mut message = if missing_mesh != 0 {
            "PlayAnim: No mesh in Object ".encode_utf16().map(u32::from).collect::<Vec<_>>()
        } else {
            "PlayAnim: Sequence ".encode_utf16().map(u32::from).collect::<Vec<_>>()
        };
        if missing_mesh != 0 {
            message.extend(actor_name);
            message.extend(" for Sequence ".encode_utf16().map(u32::from));
        }
        message.extend(label);
        if missing_mesh == 0 {
            message.extend(" not found in Mesh ".encode_utf16().map(u32::from));
            let mesh = unsafe { (*actor.cast::<AActor>()).mesh };
            if !mesh.is_null() { message.extend(context.access.name_text(i32::from_ne_bytes(unsafe { (*mesh.cast::<UObject>()).name }))?); }
        }
        message.push(0);
        context.access.emit_log(category, &message)
    })();
    context.remember(result)
}

unsafe extern "C" fn set_latent_action(_: *mut c_void, actor: *mut c_void, action: i32) -> i32 {
    let frame = unsafe { (*actor.cast::<UObject>()).state_frame.cast::<hp2_abi::engine::generated::FStateFrame>() };
    if frame.is_null() { return PLAY_ERROR; }
    unsafe { (*frame).latent_action = action; }
    0
}
pub(crate) const LATENT_SERVICES: crate::actor::Hp2ActorLatentServices =
    crate::actor::Hp2ActorLatentServices { context: ptr::null_mut(), set_latent_action };

fn animation_services(context: *mut c_void) -> actor_anim::Hp2ActorAnimServices {
    actor_anim::Hp2ActorAnimServices { context, mesh_is_skeletal, actor_skeletal_animation: linked_animation, mesh_default_animation: default_animation, find_sequence }
}
fn bone_number_services(context: *mut c_void) -> actor_anim::Hp2ActorBoneNumberServices {
    actor_anim::Hp2ActorBoneNumberServices { context, mesh_is_skeletal, bone_index }
}
fn create_services(context: *mut c_void) -> actor_anim::Hp2ActorCreateAnimChannelServices {
    actor_anim::Hp2ActorCreateAnimChannelServices { context, mesh_is_skeletal, bone_index, channel_not_replaceable, spawn_channel, destroy_channel, insert_channel, append_channel }
}
unsafe fn auxiliary(actor: *mut c_void, index: i32) -> RuntimeResult<*mut AActor> {
    let array = unsafe { ptr::addr_of!((*actor.cast::<AActor>()).aux_anims).read_unaligned() };
    if index < 0 || index >= array.num || array.data.is_null() {
        return Err(RuntimeError::new("Invalid auxiliary animation index"));
    }
    let channel = unsafe { array.data.cast::<*mut AActor>().add(index as usize).read_unaligned() };
    if channel.is_null() { return Err(RuntimeError::new("Null auxiliary animation channel")); }
    Ok(channel)
}
unsafe extern "C" fn auxiliary_count(_: *mut c_void, actor: *mut c_void, out: *mut i32) -> i32 {
    unsafe { out.write_unaligned((*actor.cast::<AActor>()).aux_anims.num); }
    0
}
unsafe extern "C" fn auxiliary_bone(ctx: *mut c_void, actor: *mut c_void, index: i32, out: *mut i32) -> i32 {
    let context = unsafe { &mut *ctx.cast::<AnimContext<'_>>() };
    context.remember(unsafe { auxiliary(actor, index) }.map(|channel| unsafe { out.write_unaligned(i32::from((*channel).anim_bone)); }))
}
unsafe extern "C" fn auxiliary_is_animating(ctx: *mut c_void, actor: *mut c_void, index: i32, out: *mut i32) -> i32 {
    let context = unsafe { &mut *ctx.cast::<AnimContext<'_>>() };
    context.remember(unsafe { auxiliary(actor, index) }.map(|channel| unsafe { out.write_unaligned(i32::from(crate::actor::is_animating(&*channel))); }))
}
unsafe extern "C" fn sequence_group(_: *mut c_void, sequence: *mut c_void, out: *mut c_void) -> i32 {
    unsafe { out.cast::<[u8; 4]>().write_unaligned((*sequence.cast::<FMeshAnimSeq>()).group); }
    0
}
unsafe extern "C" fn mesh_bone_name(_: *mut c_void, mesh: *mut c_void, number: i32, out: *mut c_void) -> i32 {
    unsafe { out.cast::<[u8; 4]>().write_unaligned(crate::skeletal::bone_name(mesh.cast(), number)); }
    0
}
unsafe extern "C" fn bone_position(ctx: *mut c_void, actor: *mut c_void, mesh: *mut c_void, bone: i32, out: *mut c_void) -> i32 {
    let context = unsafe { &mut *ctx.cast::<AnimContext<'_>>() };
    context.remember(unsafe { crate::render_owner::bone_coords(context.access, mesh, actor.cast(), bone) }
        .map(|coords| unsafe { out.cast::<hp2_abi::FVector>().write_unaligned(coords.origin); }))
}
unsafe extern "C" fn bone_rotation(ctx: *mut c_void, actor: *mut c_void, mesh: *mut c_void, bone: i32, out: *mut c_void) -> i32 {
    let context = unsafe { &mut *ctx.cast::<AnimContext<'_>>() };
    context.remember(unsafe { crate::render_owner::bone_coords(context.access, mesh, actor.cast(), bone) }.map(|coords| {
        let rotation = hp2_core::math::coords_ortho_rotation(&coords, hp2_core::math::trig_table());
        unsafe { out.cast::<[i32; 3]>().write_unaligned([rotation.pitch, rotation.yaw, rotation.roll]); }
    }))
}
unsafe extern "C" fn script_no_mesh(ctx: *mut c_void, frame: *mut c_void, which: i32) -> i32 {
    let context = unsafe { &mut *ctx.cast::<AnimContext<'_>>() };
    let text = if which == 0 { "GetAnimGroup: No mesh" } else { "HasAnim: No mesh" };
    let text: Vec<u32> = text.chars().map(u32::from).collect();
    context.remember(unsafe { hp2_core::canonical_vm_native::log_frame(context.access, frame.cast(), &text) })
}
unsafe extern "C" fn script_sequence_missing(ctx: *mut c_void, frame: *mut c_void, mesh: *mut c_void, name: *const c_void) -> i32 {
    let context = unsafe { &mut *ctx.cast::<AnimContext<'_>>() };
    let result = (|| {
        let name = unsafe { name.cast::<FName>().read_unaligned() };
        let mut text: Vec<u32> = "GetAnimGroup: Sequence '".chars().map(u32::from).collect();
        text.extend(context.access.name_text(name.index)?);
        text.extend("' not found in Mesh '".chars().map(u32::from));
        text.extend(context.access.name_text(i32::from_ne_bytes(unsafe { (*mesh.cast::<UObject>()).name }))?);
        text.push(u32::from('\''));
        unsafe { hp2_core::canonical_vm_native::log_frame(context.access, frame.cast(), &text) }
    })();
    context.remember(result)
}
fn script_log_services(context: *mut c_void) -> actor_anim::Hp2ScriptAnimLogServices {
    actor_anim::Hp2ScriptAnimLogServices { context, sequence_not_found: script_sequence_missing, no_mesh: script_no_mesh }
}

pub(crate) unsafe fn create_channel(access: CoreAccess<'_>, actor: *mut AActor, class: *mut UClass, animation_type: i32, root: FName, transient: i32, protected: i32) -> RuntimeResult<*mut AActor> {
    let mut context = AnimContext { access, channel_class: ptr::null_mut(), error: None };
    let services = create_services((&raw mut context).cast());
    let mut out = ptr::null_mut();
    let status = unsafe { actor_anim::exec_create_anim_channel(actor.cast(), class.cast(), animation_type, (&root as *const FName).cast_mut().cast(), transient, protected, &mut out, &services) };
    take_error(&mut context, status)?;
    Ok(out.cast())
}
pub(crate) unsafe fn finish_anim(access: CoreAccess<'_>, actor: *mut AActor, bone: FName, action: i32) -> RuntimeResult<()> {
    let mut context = AnimContext { access, channel_class: ptr::null_mut(), error: None };
    let services = bone_number_services((&raw mut context).cast());
    let status = unsafe { actor_anim::exec_finish_anim(actor.cast(), (&bone as *const FName).cast_mut().cast(), action, &services, &LATENT_SERVICES) };
    take_error(&mut context, status)
}
pub(crate) unsafe fn is_animating(access: CoreAccess<'_>, actor: *mut AActor, bone: FName) -> RuntimeResult<i32> {
    let mut context = AnimContext { access, channel_class: ptr::null_mut(), error: None };
    let ctx = (&raw mut context).cast();
    let mut out = 0;
    let status = unsafe { actor_anim::exec_is_animating(
        actor.cast(), (&bone as *const FName).cast_mut().cast(), &mut out,
        || Ok(bone_number_services(ctx)),
        || Ok(actor_anim::Hp2ActorAnimChannelServices { context: ctx, auxiliary_count, auxiliary_bone, auxiliary_is_animating }),
    ) };
    take_error(&mut context, status)?;
    Ok(out)
}
pub(crate) unsafe fn get_anim_group(access: CoreAccess<'_>, actor: *mut AActor, name: FName, frame: *mut hp2_abi::engine::generated::FFrame, out: *mut c_void) -> RuntimeResult<()> {
    let mut context = AnimContext { access, channel_class: ptr::null_mut(), error: None };
    let ctx = (&raw mut context).cast();
    let status = unsafe { actor_anim::exec_get_anim_group(
        actor.cast(), (&name as *const FName).cast_mut().cast(), frame.cast(), out,
        || Ok(animation_services(ctx)), || Ok(actor_anim::Hp2ActorAnimGroupServices { context: ctx, sequence_group }),
        || Ok(script_log_services(ctx)),
    ) };
    take_error(&mut context, status)
}
pub(crate) unsafe fn has_anim(access: CoreAccess<'_>, actor: *mut AActor, name: FName, frame: *mut hp2_abi::engine::generated::FFrame, out: *mut c_void) -> RuntimeResult<()> {
    let mut context = AnimContext { access, channel_class: ptr::null_mut(), error: None };
    let ctx = (&raw mut context).cast();
    let status = unsafe { actor_anim::exec_has_anim(
        actor.cast(), (&name as *const FName).cast_mut().cast(), frame.cast(), out,
        || Ok(animation_services(ctx)), || Ok(script_log_services(ctx)),
    ) };
    take_error(&mut context, status)
}
pub(crate) unsafe fn bone_number(access: CoreAccess<'_>, actor: *mut AActor, bone: FName) -> RuntimeResult<i32> {
    let mut context = AnimContext { access, channel_class: ptr::null_mut(), error: None };
    let services = bone_number_services((&raw mut context).cast());
    let mut out = 0;
    let status = unsafe { actor_anim::bone_number((*actor).mesh, (&bone as *const FName).cast_mut().cast(), &mut out, &services) };
    take_error(&mut context, status)?;
    Ok(out)
}
pub(crate) unsafe fn bone_name(access: CoreAccess<'_>, actor: *mut AActor, number: i32) -> RuntimeResult<FName> {
    let mut context = AnimContext { access, channel_class: ptr::null_mut(), error: None };
    let services = actor_anim::Hp2ActorBoneNameServices { context: (&raw mut context).cast(), mesh_is_skeletal, bone_name: mesh_bone_name };
    let mut out = FName { index: 0 };
    let status = unsafe { actor_anim::bone_name((*actor).mesh, number, (&raw mut out).cast(), &services) };
    take_error(&mut context, status)?;
    Ok(out)
}
pub(crate) unsafe fn bone_transform(access: CoreAccess<'_>, actor: *mut AActor, name: FName, kind: i32, out: *mut c_void) -> RuntimeResult<()> {
    let mut context = AnimContext { access, channel_class: ptr::null_mut(), error: None };
    let services = actor_anim::Hp2ActorBoneTransformServices { context: (&raw mut context).cast(), mesh_is_skeletal, bone_index, bone_position, bone_rotation };
    let status = unsafe { actor_anim::bone_transform(actor.cast(), (*actor).mesh, (&name as *const FName).cast_mut().cast(), kind, out, &services) };
    take_error(&mut context, status)
}
