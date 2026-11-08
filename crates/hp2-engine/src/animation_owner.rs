//! Canonical class operations for Engine animation and gesture resources.

use hp2_abi::engine::generated::{self, FAnalogTrack, FAnimVec, FMeshAnimNotify, FMeshAnimSeq, FNamedBone, MotionChunk, TArrayRaw, TRefArrayRaw, UAnimation, UGesture, UObject};
use hp2_core::array_archive as array_wire;
use hp2_core::array_storage::{self, Hp2ArrayServices, Hp2ArraySlots};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind};
use hp2_core::object_archive::{ArchiveMode, ObjectArchive};
use hp2_core::object_owner::ClassOps;
use std::{alloc::Layout, ffi::c_void, mem::size_of, ptr};

const OBJECT: ClassOps = NativeRecordKind::Object.operations();
const ARRAYS: Hp2ArrayServices = array_storage::system_allocator();

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    spec("Animation", Layout::new::<UAnimation>(), ANIMATION),
    spec("Gesture", Layout::new::<UGesture>(), GESTURE),
];

const fn spec(name: &'static str, layout: Layout, operations: ClassOps) -> NativeClassSpec {
    NativeClassSpec {
        short_name: name,
        package: "Engine",
        super_name: Some("Object"),
        within: "Object",
        flags: 0,
        object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(layout),
        operations: Some(operations),
        properties: &[],
        defaults: &[],
    }
}

const ANIMATION: ClassOps = {
    let mut operations = OBJECT;
    operations.construct = construct_animation;
    operations.finalize_members = finalize_animation;
    operations.serialize = serialize_animation;
    operations.visit_references = visit_animation;
    operations
};
const GESTURE: ClassOps = {
    let mut operations = OBJECT;
    operations.construct = construct_gesture;
    operations.finalize_members = finalize_gesture;
    operations.serialize = serialize_gesture;
    operations.visit_references = visit_gesture;
    operations
};

fn error(message: &'static str) -> RuntimeError { RuntimeError::new(message) }
fn storage_error(_: i32, what: &'static str) -> RuntimeError { error(what) }

unsafe fn slots(raw: *mut TArrayRaw) -> Hp2ArraySlots {
    Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*raw).data) },
        num: unsafe { ptr::addr_of_mut!((*raw).num) },
        max: unsafe { ptr::addr_of_mut!((*raw).max) },
    }
}

unsafe fn construct_animation(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (OBJECT.construct)(access, object)?; }
    let animation = object.cast::<UAnimation>();
    unsafe {
        initialize_array(ptr::addr_of_mut!((*animation).ref_bones));
        initialize_array(ptr::addr_of_mut!((*animation).master_track.key_quat));
        initialize_array(ptr::addr_of_mut!((*animation).master_track.key_pos));
        initialize_array(ptr::addr_of_mut!((*animation).master_track.key_delta));
        initialize_array(ptr::addr_of_mut!((*animation).moves));
        initialize_array(ptr::addr_of_mut!((*animation).anim_seqs));
        initialize_array(ptr::addr_of_mut!((*animation).moves_info));
        initialize_array(ptr::addr_of_mut!((*animation).raw_anim_keys));
        initialize_array(ptr::addr_of_mut!((*animation).raw_anim_seq_info));
        ptr::addr_of_mut!((*animation).comp_factor).write_unaligned(1.0);
    }
    Ok(())
}

unsafe fn construct_gesture(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (OBJECT.construct)(access, object)?; }
    let gesture = object.cast::<UGesture>();
    unsafe {
        initialize_array(ptr::addr_of_mut!((*gesture).points));
        initialize_array(ptr::addr_of_mut!((*gesture).segments));
    }
    Ok(())
}

unsafe fn initialize_array(raw: *mut TArrayRaw) {
    unsafe { raw.write_unaligned(TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }); }
}

unsafe fn finalize_animation(_access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let animation = object.cast::<UAnimation>();
    let mut result = Ok(());
    unsafe {
        result = combine(result, destroy_array(ptr::addr_of_mut!((*animation).raw_anim_seq_info), size_of::<generated::AnimInfoBinary>(), None));
        result = combine(result, destroy_array(ptr::addr_of_mut!((*animation).raw_anim_keys), size_of::<generated::VQuatAnimKey>(), None));
        result = combine(result, destroy_array(ptr::addr_of_mut!((*animation).moves_info), size_of::<generated::MotionChunkDigestInfo>(), None));
        result = combine(result, destroy_array(ptr::addr_of_mut!((*animation).anim_seqs), size_of::<FMeshAnimSeq>(), Some(destroy_sequence)));
        result = combine(result, destroy_array(ptr::addr_of_mut!((*animation).moves), size_of::<MotionChunk>(), Some(destroy_motion)));
        result = combine(result, destroy_array(ptr::addr_of_mut!((*animation).master_track.key_delta), size_of::<u8>(), None));
        result = combine(result, destroy_array(ptr::addr_of_mut!((*animation).master_track.key_pos), size_of::<FAnimVec>(), None));
        result = combine(result, destroy_array(ptr::addr_of_mut!((*animation).master_track.key_quat), size_of::<FAnimVec>(), None));
        result = combine(result, destroy_array(ptr::addr_of_mut!((*animation).ref_bones), size_of::<FNamedBone>(), None));
    }
    result
}

unsafe fn finalize_gesture(_access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let gesture = object.cast::<UGesture>();
    let segments = unsafe { destroy_array(ptr::addr_of_mut!((*gesture).segments), size_of::<i32>(), None) };
    let points = unsafe { destroy_array(ptr::addr_of_mut!((*gesture).points), size_of::<hp2_abi::FVector>(), None) };
    segments.and(points)
}

type ElementDestroy = unsafe fn(*mut c_void) -> RuntimeResult<()>;
pub(crate) unsafe fn destroy_array(raw: *mut TArrayRaw, width: usize, destroy: Option<ElementDestroy>) -> RuntimeResult<()> {
    let slots = unsafe { slots(raw) };
    let count = unsafe { slots.num.read() };
    let data = unsafe { slots.data.read_unaligned().cast::<u8>() };
    let mut result = Ok(());
    if count >= 0 && !data.is_null() {
        if let Some(destroy) = destroy {
            for index in 0..count as usize {
                result = combine(result, unsafe { destroy(data.add(index * width).cast()) });
            }
        }
    } else if count < 0 || (count > 0 && data.is_null()) {
        result = Err(error("Invalid animation array storage"));
    }
    let released = unsafe { array_storage::release(slots, ARRAYS) }
        .map_err(|status| storage_error(status, "Animation array release failed"));
    combine(result, released)
}

unsafe fn destroy_motion(element: *mut c_void) -> RuntimeResult<()> {
    let motion = element.cast::<MotionChunk>();
    let tracks = unsafe { destroy_array(ptr::addr_of_mut!((*motion).anim_tracks), size_of::<FAnalogTrack>(), None) };
    let bone_indices = unsafe { destroy_array(ptr::addr_of_mut!((*motion).bone_indices), size_of::<i32>(), None) };
    tracks.and(bone_indices)
}
pub(crate) unsafe fn destroy_sequence(element: *mut c_void) -> RuntimeResult<()> {
    unsafe { destroy_array(ptr::addr_of_mut!((*element.cast::<FMeshAnimSeq>()).notifys), size_of::<FMeshAnimNotify>(), None) }
}


fn combine(first: RuntimeResult<()>, next: RuntimeResult<()>) -> RuntimeResult<()> {
    match (first, next) { (Err(error), _) => Err(error), (Ok(()), result) => result }
}

unsafe fn serialize_animation(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    unsafe { (OBJECT.serialize)(access, object, archive)?; }
    if matches!(archive.mode(), ArchiveMode::Reference | ArchiveMode::TagExports | ArchiveMode::TagImports) {
        return unsafe { visit_animation_names(object.cast(), archive) };
    }
    let animation = object.cast::<UAnimation>();
    unsafe {
        serialize_array(archive, ptr::addr_of_mut!((*animation).ref_bones), size_of::<FNamedBone>(), Some(serialize_bone), None, false)?;
        serialize_array(archive, ptr::addr_of_mut!((*animation).moves), size_of::<MotionChunk>(), Some(serialize_motion), Some(destroy_motion), false)?;
        serialize_array(archive, ptr::addr_of_mut!((*animation).anim_seqs), size_of::<FMeshAnimSeq>(), Some(serialize_sequence), Some(destroy_sequence), false)?;
        serialize_array(archive, ptr::addr_of_mut!((*animation).master_track.key_quat), size_of::<FAnimVec>(), Some(serialize_anim_vec), None, generated::ANIMVEC_FAST_SERIALIZE != 0)?;
        serialize_array(archive, ptr::addr_of_mut!((*animation).master_track.key_pos), size_of::<FAnimVec>(), Some(serialize_anim_vec), None, generated::ANIMVEC_FAST_SERIALIZE != 0)?;
        serialize_array(archive, ptr::addr_of_mut!((*animation).master_track.key_delta), size_of::<u8>(), Some(serialize_u8), None, generated::SCALAR_FAST_SERIALIZE != 0)?;
    }
    if archive.is_loading() {
        unsafe { bind_track_slices(animation)?; }
    }
    Ok(())
}

unsafe fn serialize_gesture(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    // Native UGesture inherits UObject serialization, including its reflected fields.
    unsafe { (OBJECT.serialize)(access, object, archive) }
}


unsafe fn bind_track_slices(animation: *mut UAnimation) -> RuntimeResult<()> {
    let master = unsafe { ptr::addr_of_mut!((*animation).master_track) };
    let mut q = 0usize;
    let mut p = 0usize;
    let mut d = 0usize;
    unsafe {
        let (moves, count) = array_parts::<MotionChunk>(ptr::addr_of!((*animation).moves))?;
        for index in 0..count {
            let motion = moves.add(index * size_of::<MotionChunk>()).cast::<MotionChunk>();
            let (tracks, track_count) = array_parts::<FAnalogTrack>(ptr::addr_of!((*motion).anim_tracks))?;
            for track_index in 0..track_count {
                let track = tracks.add(track_index * size_of::<FAnalogTrack>()).cast::<FAnalogTrack>();
                bind_ref(ptr::addr_of_mut!((*track).key_quat), ptr::addr_of_mut!((*master).key_quat), &mut q, size_of::<FAnimVec>())?;
                bind_ref(ptr::addr_of_mut!((*track).key_pos), ptr::addr_of_mut!((*master).key_pos), &mut p, size_of::<FAnimVec>())?;
                bind_ref(ptr::addr_of_mut!((*track).key_delta), ptr::addr_of_mut!((*master).key_delta), &mut d, size_of::<u8>())?;
            }
        }
    }
    Ok(())
}

unsafe fn bind_ref(reference: *mut TRefArrayRaw, master: *mut TArrayRaw, offset: &mut usize, width: usize) -> RuntimeResult<()> {
    let count = usize::try_from(unsafe { ptr::read_unaligned(ptr::addr_of!((*reference).num)) })
        .map_err(|_| error("Negative animation track key count"))?;
    let master = unsafe { ptr::read_unaligned(master) };
    let total = usize::try_from(master.num).map_err(|_| error("Negative master track key count"))?;
    let end = (*offset).checked_add(count).ok_or_else(|| error("Animation track key range overflow"))?;
    if end > total || (total > 0 && master.data.is_null()) {
        return Err(error("Animation track key range exceeds master track"));
    }
    let data = if master.data.is_null() { ptr::null_mut() }
        else { unsafe { master.data.cast::<u8>().add(*offset * width).cast() } };
    unsafe { ptr::addr_of_mut!((*reference).data).write_unaligned(data); }
    *offset = end;
    Ok(())
}

struct ArrayContext<'borrow, 'data> {
    archive: &'borrow ObjectArchive<'data>,
    slots: Hp2ArraySlots,
    width: usize,
    item: Option<unsafe fn(&ObjectArchive<'_>, *mut c_void) -> RuntimeResult<()>>,
    destroy: Option<ElementDestroy>,
    fast: bool,
    loading_count: std::cell::Cell<i32>,
    error: std::cell::RefCell<Option<RuntimeError>>,
}

pub(crate) unsafe fn serialize_array(
    archive: &ObjectArchive<'_>, raw: *mut TArrayRaw, width: usize,
    item: Option<unsafe fn(&ObjectArchive<'_>, *mut c_void) -> RuntimeResult<()>>,
    destroy: Option<ElementDestroy>,
    fast: bool,
) -> RuntimeResult<()> {
    let slots = unsafe { slots(raw) };
    let mut context = ArrayContext {
        archive, slots, width, item, destroy, fast,
        loading_count: std::cell::Cell::new(0), error: std::cell::RefCell::new(None),
    };
    // SAFETY: the callback context remains live for the synchronous shared array state machine.
    let state = unsafe {
        array_wire::run(
            (&raw mut context).cast(), ptr::null_mut(), array_operation,
            array_wire::action::ARRAY, 0, ptr::null_mut(),
        )
    };
    match state {
        Ok(()) => Ok(()),
        Err(_) => Err(context.error.into_inner().unwrap_or_else(|| error("Animation array archive failed"))),
    }
}

unsafe extern "C" fn array_operation(
    context: *mut c_void, _: *mut c_void, operation: i32, arg: i32, value: *mut c_void,
) -> i32 {
    let context = unsafe { &*context.cast::<ArrayContext<'_, '_>>() };
    let result = (|| -> RuntimeResult<()> {
        let slots = context.slots;
        match operation {
            array_wire::LOADING => unsafe { value.cast::<i32>().write(i32::from(context.archive.is_loading())) },
            array_wire::SAVING => unsafe { value.cast::<i32>().write(i32::from(context.archive.is_saving())) },
            array_wire::NUM => unsafe { value.cast::<i32>().write(slots.num.read()) },
            array_wire::FAST => unsafe { value.cast::<i32>().write(i32::from(context.fast)) },
            array_wire::NEEDS_DESTRUCTOR => unsafe { value.cast::<i32>().write(i32::from(context.destroy.is_some())) },
            array_wire::COUNT_BYTES => {
                let count = usize::try_from(unsafe { slots.num.read() }).map_err(|_| error("Negative animation array count"))?;
                let max = usize::try_from(unsafe { slots.max.read() }).map_err(|_| error("Negative animation array capacity"))?;
                let used = count.checked_mul(context.width).ok_or_else(|| error("Animation array size overflow"))?;
                let capacity = max.checked_mul(context.width).ok_or_else(|| error("Animation array capacity overflow"))?;
                context.archive.count_bytes(used, capacity)?;
            }
            array_wire::COUNT_INDEX if context.archive.is_loading() => {
                let mut count = 0;
                context.archive.serialize_compact_index(&mut count)?;
                if count < 0 { return Err(error("Negative animation array count")); }
                context.loading_count.set(count);
            }
            array_wire::COUNT_INDEX => context.archive.serialize_compact_index(unsafe { &mut *slots.num })?,
            array_wire::COMPACT => context.archive.serialize_compact_index(unsafe { &mut *value.cast::<i32>() })?,
            array_wire::SET_EXACT => {
                let count = context.loading_count.get();
                allocate_zeroed(slots, count, context.width)?;
            }
            array_wire::RAW_SERIALIZE => {
                let count = usize::try_from(unsafe { slots.num.read() }).map_err(|_| error("Negative animation array count"))?;
                let bytes = count.checked_mul(context.width).ok_or_else(|| error("Animation array size overflow"))?;
                let bytes_i32 = i32::try_from(bytes).map_err(|_| error("Animation array exceeds native byte count"))?;
                let data = unsafe { slots.data.read_unaligned().cast::<u8>() };
                if bytes_i32 != 0 && data.is_null() { return Err(error("Nonempty animation array has no storage")); }
                if context.archive.is_loading() {
                    unsafe { context.archive.read_raw(data, bytes)?; }
                } else if context.archive.is_saving() {
                    let data = if bytes == 0 { &[] } else { unsafe { std::slice::from_raw_parts(data, bytes) } };
                    context.archive.write(data)?;
                }
            }
            array_wire::DESTRUCT => {
                let destroy = context.destroy.ok_or_else(|| error("Animation array element has no destructor"))?;
                let count = unsafe { slots.num.read() };
                if arg < 0 || arg >= count { return Err(error("Animation array destructor index is invalid")); }
                let data = unsafe { slots.data.read_unaligned().cast::<u8>() };
                if data.is_null() { return Err(error("Nonempty animation array has no storage")); }
                unsafe { destroy(data.add(arg as usize * context.width).cast())?; }
            }
            array_wire::EMPTY_STORAGE => {
                unsafe { slots.num.write(0); }
                allocate_zeroed(slots, arg, context.width)?;
                unsafe { slots.num.write(0); }
            }
            array_wire::CONSTRUCT_SERIALIZE => {
                let item = context.item.ok_or_else(|| error("Animation array element has no serializer"))?;
                let width = i32::try_from(context.width).map_err(|_| error("Animation element width overflow"))?;
                let index = unsafe { array_storage::add(slots, 1, width, ptr::null(), ARRAYS) }
                    .map_err(|status| storage_error(status, "Animation array growth failed"))?;
                let data = unsafe { slots.data.read_unaligned().cast::<u8>() };
                if data.is_null() { return Err(error("Animation array allocation returned null")); }
                let element = unsafe { data.add(index as usize * context.width) };
                unsafe { element.write_bytes(0, context.width); }
                unsafe { item(context.archive, element.cast())?; }
            }
            array_wire::ITEM_SERIALIZE => {
                let item = context.item.ok_or_else(|| error("Animation array element has no serializer"))?;
                let count = unsafe { slots.num.read() };
                if arg < 0 || arg >= count { return Err(error("Animation array item index is invalid")); }
                let data = unsafe { slots.data.read_unaligned().cast::<u8>() };
                if data.is_null() { return Err(error("Nonempty animation array has no storage")); }
                unsafe { item(context.archive, data.add(arg as usize * context.width).cast())?; }
            }
            _ => return Err(error("Unsupported animation array operation")),
        }
        Ok(())
    })();
    match result {
        Ok(()) => 0,
        Err(error) => {
            *context.error.borrow_mut() = Some(error);
            1
        }
    }
}

unsafe fn allocate_zeroed(slots: Hp2ArraySlots, count: i32, width: usize) -> RuntimeResult<()> {
    if count < 0 { return Err(error("Negative animation array count")); }
    let width_i32 = i32::try_from(width).map_err(|_| error("Animation element width overflow"))?;
    unsafe { array_storage::release(slots, ARRAYS) }
        .map_err(|status| storage_error(status, "Animation array release failed"))?;
    unsafe { array_storage::set_exact(slots, count, width_i32, ptr::null(), ARRAYS) }
        .map_err(|status| {
            unsafe {
                slots.num.write(0);
                slots.max.write(0);
                slots.data.write_unaligned(ptr::null_mut());
            }
            storage_error(status, "Animation array allocation failed")
        })?;
    let bytes = (count as usize).checked_mul(width).ok_or_else(|| error("Animation array size overflow"))?;
    if bytes != 0 {
        let data = unsafe { slots.data.read_unaligned().cast::<u8>() };
        if data.is_null() { return Err(error("Animation array allocation returned null")); }
        unsafe { data.write_bytes(0, bytes); }
    }
    Ok(())
}

unsafe fn array_parts<T>(raw: *const TArrayRaw) -> RuntimeResult<(*mut u8, usize)> {
    let value = unsafe { ptr::read_unaligned(raw) };
    if value.num < 0 || (value.num > 0 && value.data.is_null()) {
        return Err(error("Invalid animation array storage"));
    }
    Ok((value.data.cast(), value.num as usize))
}

unsafe fn serialize_bone(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    let bone = value.cast::<FNamedBone>();
    unsafe { archive.serialize_name_slot(ptr::addr_of_mut!((*bone).name).cast())?; }
    unsafe { archive.serialize_u32(&mut *ptr::addr_of_mut!((*bone).flags))?; }
    unsafe { archive.serialize_i32(&mut *ptr::addr_of_mut!((*bone).parent_index)) }
}

unsafe fn serialize_motion(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    let motion = value.cast::<MotionChunk>();
    unsafe {
        serialize_vector(archive, ptr::addr_of_mut!((*motion).root_speed3_d))?;
        archive.serialize_f32(&mut *ptr::addr_of_mut!((*motion).track_time))?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*motion).start_bone))?;
        archive.serialize_u32(&mut *ptr::addr_of_mut!((*motion).flags))?;
        serialize_array(archive, ptr::addr_of_mut!((*motion).bone_indices), size_of::<i32>(), Some(serialize_i32), None, generated::SCALAR_FAST_SERIALIZE != 0)?;
        serialize_array(archive, ptr::addr_of_mut!((*motion).anim_tracks), size_of::<FAnalogTrack>(), Some(serialize_track), None, false)
    }
}

unsafe fn serialize_track(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    let track = value.cast::<FAnalogTrack>();
    unsafe {
        archive.serialize_u32(&mut *ptr::addr_of_mut!((*track).flags))?;
        serialize_ref_count(archive, ptr::addr_of_mut!((*track).key_quat))?;
        serialize_ref_count(archive, ptr::addr_of_mut!((*track).key_pos))?;
        serialize_ref_count(archive, ptr::addr_of_mut!((*track).key_delta))?;
        archive.serialize_f32(&mut *ptr::addr_of_mut!((*track).pos_scale))?;
        archive.serialize_f32(&mut *ptr::addr_of_mut!((*track).time_scale))
    }
}

pub(crate) unsafe fn serialize_sequence(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    let sequence = value.cast::<FMeshAnimSeq>();
    unsafe {
        archive.serialize_name_slot(ptr::addr_of_mut!((*sequence).name).cast())?;
        archive.serialize_name_slot(ptr::addr_of_mut!((*sequence).group).cast())?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*sequence).start_frame))?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*sequence).num_frames))?;
        serialize_array(archive, ptr::addr_of_mut!((*sequence).notifys), size_of::<FMeshAnimNotify>(), Some(serialize_notify), None, false)?;
        archive.serialize_f32(&mut *ptr::addr_of_mut!((*sequence).rate))
    }
}

unsafe fn serialize_notify(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    let notify = value.cast::<FMeshAnimNotify>();
    unsafe {
        archive.serialize_f32(&mut *ptr::addr_of_mut!((*notify).time))?;
        archive.serialize_name_slot(ptr::addr_of_mut!((*notify).function).cast())
    }
}

unsafe fn serialize_anim_vec(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    let vector = value.cast::<FAnimVec>();
    unsafe {
        archive.serialize_u16(&mut *ptr::addr_of_mut!((*vector).x).cast())?;
        archive.serialize_u16(&mut *ptr::addr_of_mut!((*vector).y).cast())?;
        archive.serialize_u16(&mut *ptr::addr_of_mut!((*vector).z).cast())
    }
}

unsafe fn serialize_vector(archive: &ObjectArchive<'_>, value: *mut hp2_abi::FVector) -> RuntimeResult<()> {
    unsafe {
        archive.serialize_f32(&mut *ptr::addr_of_mut!((*value).x))?;
        archive.serialize_f32(&mut *ptr::addr_of_mut!((*value).y))?;
        archive.serialize_f32(&mut *ptr::addr_of_mut!((*value).z))
    }
}

unsafe fn serialize_i32(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    unsafe { archive.serialize_i32(&mut *value.cast()) }
}
unsafe fn serialize_u8(archive: &ObjectArchive<'_>, value: *mut c_void) -> RuntimeResult<()> {
    unsafe { archive.serialize_u8(&mut *value.cast()) }
}
unsafe fn serialize_ref_count(archive: &ObjectArchive<'_>, raw: *mut TRefArrayRaw) -> RuntimeResult<()> {
    let mut value = unsafe { ptr::read_unaligned(ptr::addr_of!((*raw).num)) };
    archive.serialize_compact_index(&mut value)?;
    if value < 0 { return Err(error("Negative animation track key count")); }
    if archive.is_loading() {
        unsafe {
            ptr::addr_of_mut!((*raw).num).write_unaligned(value);
            ptr::addr_of_mut!((*raw).data).write_unaligned(ptr::null_mut());
        }
    }
    Ok(())
}

unsafe fn visit_animation(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    unsafe {
        (OBJECT.visit_references)(access, object, archive)?;
        visit_animation_names(object.cast(), archive)
    }
}

unsafe fn visit_animation_names(animation: *mut UAnimation, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    unsafe {
        let (bones, bone_count) = array_parts::<FNamedBone>(ptr::addr_of!((*animation).ref_bones))?;
        for index in 0..bone_count {
            let bone = bones.add(index * size_of::<FNamedBone>()).cast::<FNamedBone>();
            archive.serialize_name_slot(ptr::addr_of_mut!((*bone).name).cast())?;
        }
        let (sequences, sequence_count) = array_parts::<FMeshAnimSeq>(ptr::addr_of!((*animation).anim_seqs))?;
        for index in 0..sequence_count {
            let sequence = sequences.add(index * size_of::<FMeshAnimSeq>()).cast::<FMeshAnimSeq>();
            archive.serialize_name_slot(ptr::addr_of_mut!((*sequence).name).cast())?;
            archive.serialize_name_slot(ptr::addr_of_mut!((*sequence).group).cast())?;
            let (notifies, notify_count) = array_parts::<FMeshAnimNotify>(ptr::addr_of!((*sequence).notifys))?;
            for notify_index in 0..notify_count {
                let notify = notifies.add(notify_index * size_of::<FMeshAnimNotify>()).cast::<FMeshAnimNotify>();
                archive.serialize_name_slot(ptr::addr_of_mut!((*notify).function).cast())?;
            }
        }
    }
    Ok(())
}
unsafe fn visit_gesture(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    unsafe { (OBJECT.visit_references)(access, object, archive) }
}
