//! Canonical Rust ClassOps for Engine.Sound and its owned audio storage.

use crate::{audio, core_owner::EngineState};
use hp2_abi::engine::generated::{self, FSoundData, FLipSyncData, TArrayRaw, UObject, USound};
use hp2_core::{
    array_storage::{self, Hp2ArrayServices, Hp2ArraySlots},
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_class_specs::{NativeClassSpec, NativeRecordKind},
    lazy_array::{self, LazyArraySlots},
    name_owner::FindName,
    object_archive::{ArchiveMode, ObjectArchive},
    object_owner::ClassOps,
};
use std::{alloc::Layout, ffi::c_void, mem::size_of, ptr};

const OBJECT: ClassOps = NativeRecordKind::Object.operations();
const ARRAYS: Hp2ArrayServices = array_storage::system_allocator();
const HAS_LIP_SYNC: i32 = 128;
const SOUND: ClassOps = {
    let mut operations = OBJECT;
    operations.construct = construct_sound;
    operations.destroy = destroy_sound;
    operations.finalize_members = finalize_members;
    operations.serialize = serialize_sound;
    operations.visit_references = visit_sound;
    operations
};

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "Sound",
    package: "Engine",
    super_name: Some("Object"),
    within: "Object",
    flags: generated::SOUND_CLASS_FLAGS,
    object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
    config: "System",
    guid: [0; 4],
    kind: NativeRecordKind::Object,
    layout: Some(native_layout::<USound>(USound::NATIVE_ALIGNMENT)),
    operations: Some(SOUND),
    properties: &[],
    defaults: &[],
}, NativeClassSpec {
    short_name: "Music",
    package: "Engine",
    super_name: Some("Object"),
    within: "Object",
    flags: 0,
    object_flags: 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000,
    config: "System",
    guid: [0; 4],
    kind: NativeRecordKind::Object,
    layout: Some(native_layout::<generated::UMusic>(generated::UMusic::NATIVE_ALIGNMENT)),
    operations: Some(OBJECT),
    properties: &[],
    defaults: &[],
}];

const fn native_layout<T>(alignment: usize) -> Layout {
    match Layout::from_size_align(size_of::<T>(), alignment) {
        Ok(layout) => layout,
        Err(_) => panic!("compiler-probed native alignment is invalid"),
    }
}

fn error(message: &'static str) -> RuntimeError {
    RuntimeError::new(message)
}

fn combine(first: RuntimeResult<()>, next: RuntimeResult<()>) -> RuntimeResult<()> {
    match (first, next) {
        (Err(error), _) => Err(error),
        (Ok(()), result) => result,
    }
}

unsafe fn array_slots(array: *mut TArrayRaw) -> Hp2ArraySlots {
    Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*array).data) },
        num: unsafe { ptr::addr_of_mut!((*array).num) },
        max: unsafe { ptr::addr_of_mut!((*array).max) },
    }
}

unsafe fn construct_sound(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (OBJECT.construct)(access, object)?; }
    let sound = object.cast::<USound>();
    unsafe {
        ptr::addr_of_mut!((*sound).duration).write_unaligned(-1.0);
        ptr::addr_of_mut!((*sound).core_flags).write_unaligned(0);
        ptr::addr_of_mut!((*sound).ref_count).write_unaligned(0);
        ptr::addr_of_mut!((*sound).raw_num_samples).write_unaligned(0);
        ptr::addr_of_mut!((*sound).raw_bits_per_sample).write_unaligned(0);
        ptr::addr_of_mut!((*sound).raw_num_channels).write_unaligned(0);
        ptr::addr_of_mut!((*sound).raw_sample_rate).write_unaligned(0);
        ptr::addr_of_mut!((*sound).data.owner).write_unaligned(object.cast());
        ptr::addr_of_mut!((*sound).lip_sync_data.owner).write_unaligned(object.cast());
        ptr::addr_of_mut!((*sound).lip_sync_data.m_i_max_amplitude).write_unaligned(-1);
        initialize_array(ptr::addr_of_mut!((*sound).open_streams));
        initialize_array(ptr::addr_of_mut!((*sound).filename));
        lazy_array::lazy_array_initialize(access, lazy_slots_data(ptr::addr_of_mut!((*sound).data)))?;
        if let Err(init_error) = lazy_array::lazy_array_initialize(access, lazy_slots_lip(ptr::addr_of_mut!((*sound).lip_sync_data))) {
            let _ = destroy_lazy(access, lazy_slots_data(ptr::addr_of_mut!((*sound).data)));
            return Err(init_error);
        }
        if let Err(init_error) = hp2_core::map_storage::initialize_owned_pod(ptr::addr_of_mut!((*sound).playing_streams).cast()) {
            let _ = destroy_sound_members(access, sound);
            return Err(init_error);
        }
    }
    Ok(())
}

unsafe fn initialize_array(raw: *mut TArrayRaw) {
    unsafe {
        raw.write_unaligned(TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 });
    }
}

unsafe fn destroy_sound(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let sound = object.cast::<USound>();
    audio::usound_destroy_policy(
        || {
            let state = access.owner_context::<EngineState>()
                .ok_or_else(|| error("Sound class operations require their Engine owner context"))?;
            match state.sound_audio.get() {
                None => Ok(()),
                Some(binding) => unsafe {
                    (binding.unregister_sound)(access, binding.receiver, ptr::NonNull::new_unchecked(sound))
                },
            }
        },
        || unsafe { (OBJECT.destroy)(access, object) },
    )
}

unsafe fn finalize_members(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(error("Null canonical USound")); }
    unsafe { destroy_sound_members(access, object.cast()) }
}

unsafe fn destroy_sound_members(access: CoreAccess<'_>, sound: *mut USound) -> RuntimeResult<()> {
    let mut result = Ok(());
    let filename = unsafe { release_array(ptr::addr_of_mut!((*sound).filename), "Sound filename storage release failed") };
    result = combine(result, filename);
    unsafe { hp2_core::map_storage::destroy_owned_pod(ptr::addr_of_mut!((*sound).playing_streams).cast()); }
    let streams = unsafe { release_array(ptr::addr_of_mut!((*sound).open_streams), "Sound stream storage release failed") };
    result = combine(result, streams);
    let lip = unsafe { destroy_lazy(access, lazy_slots_lip(ptr::addr_of_mut!((*sound).lip_sync_data))) };
    result = combine(result, lip);
    let data = unsafe { destroy_lazy(access, lazy_slots_data(ptr::addr_of_mut!((*sound).data))) };
    combine(result, data)
}

unsafe fn destroy_lazy(access: CoreAccess<'_>, slots: LazyArraySlots) -> RuntimeResult<()> {
    let detached = unsafe { lazy_array::lazy_array_destroy(access, slots) };
    let released = unsafe { array_storage::release(array_slots(slots.array), ARRAYS) }
        .map_err(|_| error("Sound lazy byte storage release failed"));
    combine(detached, released)
}

unsafe fn release_array(raw: *mut TArrayRaw, message: &'static str) -> RuntimeResult<()> {
    unsafe { array_storage::release(array_slots(raw), ARRAYS) }.map_err(|_| error(message))
}

unsafe fn serialize_sound(
    access: CoreAccess<'_>,
    object: *mut UObject,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    let sound = object.cast::<USound>();
    unsafe { (OBJECT.serialize)(access, object, archive)?; }
    if matches!(
        archive.mode(),
        ArchiveMode::Reference | ArchiveMode::TagExports | ArchiveMode::TagImports
    ) {
        return unsafe { archive.serialize_name_slot(ptr::addr_of_mut!((*sound).file_type).cast()) };
    }
    unsafe {
        archive.serialize_name_slot(ptr::addr_of_mut!((*sound).file_type).cast())?;
        archive.serialize_i32(&mut *ptr::addr_of_mut!((*sound).core_flags))?;
        archive.serialize_f32(&mut *ptr::addr_of_mut!((*sound).duration))?;
        audio::sound_serialize_raw_fields(i32::from(archive.flags().version), |field| {
            let value = match field {
                audio::SoundRawField::NumSamples => ptr::addr_of_mut!((*sound).raw_num_samples),
                audio::SoundRawField::BitsPerSample => ptr::addr_of_mut!((*sound).raw_bits_per_sample),
                audio::SoundRawField::NumChannels => ptr::addr_of_mut!((*sound).raw_num_channels),
                audio::SoundRawField::SampleRate => ptr::addr_of_mut!((*sound).raw_sample_rate),
            };
            archive.serialize_i32(&mut *value)
        })?;
        if matches!(archive.mode(), ArchiveMode::Loading | ArchiveMode::Saving | ArchiveMode::Transaction) {
            lazy_array::lazy_array_serialize(access, object, lazy_slots_data(ptr::addr_of_mut!((*sound).data)), archive, 1, serialize_bytes)?;
            if (*sound).core_flags & HAS_LIP_SYNC != 0 {
                lazy_array::lazy_array_serialize(access, object, lazy_slots_lip(ptr::addr_of_mut!((*sound).lip_sync_data)), archive, 1, serialize_bytes)?;
            }
        } else {
            let size = usize::try_from((*sound).original_size).map_err(|_| error("Invalid Sound original size"))?;
            archive.count_bytes(size, size)?;
        }
    }
    Ok(())
}

unsafe fn serialize_bytes(
    _: CoreAccess<'_>,
    array: *mut c_void,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe {
        crate::animation_owner::serialize_array(
            archive,
            array.cast(),
            1,
            None,
            None,
            generated::SCALAR_FAST_SERIALIZE != 0,
        )
    }
}

unsafe fn visit_sound(
    access: CoreAccess<'_>,
    object: *mut UObject,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe {
        (OBJECT.visit_references)(access, object, archive)?;
        archive.serialize_name_slot(ptr::addr_of_mut!((*object.cast::<USound>()).file_type).cast())
    }
}

unsafe fn lazy_slots_data(data: *mut FSoundData) -> LazyArraySlots {
    unsafe { lazy_slots(data.cast(), generated::FSoundData::TLAZYARRAY_BYTE_OFFSET) }
}

unsafe fn lazy_slots_lip(lip: *mut FLipSyncData) -> LazyArraySlots {
    unsafe { lazy_slots(lip.cast(), generated::FLipSyncData::TLAZYARRAY_BYTE_OFFSET) }
}

unsafe fn lazy_slots(record: *mut u8, lazy_offset: usize) -> LazyArraySlots {
    let lazy = unsafe { record.add(lazy_offset).cast::<generated::TLazyBytes>() };
    LazyArraySlots {
        array: unsafe { lazy.cast::<u8>().add(generated::TLazyBytes::TARRAY_BYTE_OFFSET).cast() },
        loader: unsafe { lazy.cast::<u8>().add(generated::TLazyBytes::FLAZYLOADER_OFFSET).cast() },
        saved_archive: unsafe { ptr::addr_of_mut!((*lazy).saved_ar) },
        saved_position: unsafe { ptr::addr_of_mut!((*lazy).saved_pos) },
    }
}

/// Evaluates `USound::GetDuration` on a canonical Core Sound, materializing its owned lazy data
/// only when the cached duration is negative.
///
/// # Safety
/// `sound` is a live canonical Sound record owned by `access`.
pub(crate) unsafe fn get_duration(
    access: CoreAccess<'_>,
    sound: *mut USound,
) -> RuntimeResult<f32> {
    if sound.is_null() {
        return Err(error("Null canonical Sound duration receiver"));
    }
    let duration = unsafe { ptr::addr_of!((*sound).duration).read_unaligned() };
    if !(duration < 0.0) {
        return Ok(duration);
    }
    let data = unsafe { ptr::addr_of_mut!((*sound).data).cast::<FSoundData>() };
    let mut ops = CoreSoundDurationOps { access };
    let duration =
        unsafe { audio::sound_data_get_period_with_ops(data.cast(), &mut ops) }
            .map_err(|failure| failure.with_context("Computing canonical Sound duration"))?;
    unsafe { ptr::addr_of_mut!((*sound).duration).write_unaligned(duration); }
    Ok(duration)
}

struct CoreSoundDurationOps<'a> {
    access: CoreAccess<'a>,
}

impl audio::SoundDurationOps for CoreSoundDurationOps<'_> {
    type Error = RuntimeError;

    unsafe fn load(&mut self, data: *mut audio::SoundDataRaw) -> RuntimeResult<()> {
        let sound = unsafe { (*data).owner };
        if sound.is_null() {
            return Err(error("Canonical Sound data has no owner"));
        }
        unsafe {
            lazy_array::lazy_array_load(
                self.access,
                lazy_slots_data(ptr::addr_of_mut!((*sound).data)),
            )
        }
    }

    unsafe fn is_ps2(&mut self, sound: *mut USound) -> RuntimeResult<bool> {
        if sound.is_null() {
            return Err(error("Canonical Sound data has no owner"));
        }
        let ps2 = self.access.intern_str("PS2", FindName::Find)?;
        if ps2 == 0 {
            return Ok(false);
        }
        let raw = unsafe { ptr::addr_of!((*sound).file_type).read_unaligned() };
        Ok(i32::from_ne_bytes(raw) == ps2)
    }
}
