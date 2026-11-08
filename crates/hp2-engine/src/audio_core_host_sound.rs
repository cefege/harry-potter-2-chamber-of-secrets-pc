use std::ptr;

use hp2_abi::engine::generated::{TArrayRaw, UClass, UObject, USound};
use hp2_audio::host::{Host, HostResult, SoundInfo, WaveInfo};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::name_owner::FindName;

use super::CoreHost;

fn core_result<T>(host: &mut CoreHost<'_>, result: RuntimeResult<T>) -> HostResult<T> {
    result.map_err(|error| host.fail_with(error))
}

fn sound_ptr(host: &mut CoreHost<'_>, raw: usize) -> HostResult<*mut USound> {
    if raw == 0 {
        return Err(host.fail_with(RuntimeError::new("Null canonical USound")));
    }
    Ok(raw as *mut USound)
}

fn text_from_units(units: &[u32]) -> String {
    units
        .iter()
        .copied()
        .take_while(|unit| *unit != 0)
        .map(|unit| char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

fn fstring_text(array: TArrayRaw) -> Result<String, RuntimeError> {
    if array.num < 0 || (array.num > 0 && array.data.is_null()) {
        return Err(RuntimeError::new("Invalid canonical USound filename storage"));
    }
    if array.num == 0 {
        return Ok(String::new());
    }
    // The canonical array stores native TCHAR code units; Core's FString text
    // API speaks the same 32-bit units.
    let units = unsafe { std::slice::from_raw_parts(array.data.cast::<hp2_core::ansi::Tchar>(), array.num as usize) };
    Ok(text_from_units(hp2_core::native_text::code_units(units)))
}

/// Builds Core's typed lazy-array descriptor for the measured FSoundData layout.
///
/// # Safety
/// `data` must point to the embedded FSoundData of a live canonical USound.
pub(super) unsafe fn lazy_sound_slots(
    data: *mut crate::audio::SoundDataRaw,
) -> hp2_core::lazy_array::LazyArraySlots {
    use hp2_abi::engine::generated::FLazyLoader;
    unsafe {
        let loader = data.cast::<FLazyLoader>();
        hp2_core::lazy_array::LazyArraySlots {
            array: ptr::addr_of_mut!((*data).lazy.array).cast::<TArrayRaw>(),
            loader: loader.cast(),
            saved_archive: ptr::addr_of_mut!((*loader).saved_ar),
            saved_position: ptr::addr_of_mut!((*loader).saved_pos),
        }
    }
}

fn array_slots(array: *mut TArrayRaw) -> hp2_core::array_storage::Hp2ArraySlots {
    unsafe {
        hp2_core::array_storage::Hp2ArraySlots {
            data: ptr::addr_of_mut!((*array).data),
            num: ptr::addr_of_mut!((*array).num),
            max: ptr::addr_of_mut!((*array).max),
        }
    }
}

fn core_array_error(error: i32, context: &'static str) -> RuntimeError {
    RuntimeError::new(context).with_context(&format!("Core array error {error}"))
}

type SoundStreamPair = hp2_abi::engine::generated::TMapIntIntPair;

struct SoundMapContext {
    map: *mut hp2_core::core_records::MapStorageRaw,
    key: i32,
    value: i32,
}

unsafe fn map_pair(context: *mut std::ffi::c_void, index: i32) -> *mut SoundStreamPair {
    let state = unsafe { &*context.cast::<SoundMapContext>() };
    unsafe { (*state.map).pairs.data.cast::<SoundStreamPair>().add(index as usize) }
}

unsafe extern "C" fn sound_map_leaf(
    context: *mut std::ffi::c_void,
    operation: i32,
    index: i32,
    argument: i32,
    out: *mut i32,
) -> i32 {
    use hp2_core::map_storage::leaf;
    let state = unsafe { &mut *context.cast::<SoundMapContext>() };
    let map = unsafe { &mut *state.map };
    match operation {
        leaf::NUM => unsafe { out.write_unaligned(map.pairs.num) },
        leaf::NEXT => unsafe { out.write_unaligned((*map_pair(context, index)).hash_next) },
        leaf::LINK => unsafe { (*map_pair(context, index)).hash_next = argument },
        leaf::HASH => unsafe { out.write_unaligned((*map_pair(context, index)).key) },
        leaf::REMOVE => {
            let result = unsafe {
                hp2_core::array_storage::remove(
                    array_slots(ptr::addr_of_mut!(map.pairs)),
                    index,
                    1,
                    std::mem::size_of::<SoundStreamPair>() as i32,
                    ptr::null(),
                    hp2_core::array_storage::system_allocator(),
                )
            };
            if result.is_err() { return 1; }
        }
        leaf::EMPTY => {
            let result = unsafe {
                hp2_core::array_storage::empty(
                    array_slots(ptr::addr_of_mut!(map.pairs)),
                    0,
                    std::mem::size_of::<SoundStreamPair>() as i32,
                    ptr::null(),
                    hp2_core::array_storage::system_allocator(),
                )
            };
            if result.is_err() { return 1; }
        }
        _ => return 1,
    }
    0
}

unsafe extern "C" fn sound_map_query(
    context: *mut std::ffi::c_void,
    operation: i32,
    index: i32,
    _argument: i32,
    out: *mut i32,
) -> i32 {
    use hp2_core::map_storage::query;
    let state = unsafe { &mut *context.cast::<SoundMapContext>() };
    match operation {
        query::HASH => unsafe { out.write_unaligned(state.key) },
        query::KEY_EQ => unsafe { out.write_unaligned(i32::from((*map_pair(context, index)).key == state.key)) },
        query::CONSTRUCT => {
            let pairs = unsafe { ptr::addr_of_mut!((*state.map).pairs) };
            let added = unsafe {
                hp2_core::array_storage::add_zeroed(
                    array_slots(pairs),
                    1,
                    std::mem::size_of::<SoundStreamPair>() as i32,
                    ptr::null(),
                    hp2_core::array_storage::system_allocator(),
                )
            };
            let Ok(index) = added else { return 1; };
            unsafe {
                map_pair(context, index).write(SoundStreamPair {
                    hash_next: -1,
                    key: state.key,
                    value: state.value,
                });
                out.write_unaligned(index);
            }
        }
        query::ASSIGN => unsafe { (*map_pair(context, index)).value = state.value },
        _ => return 1,
    }
    0
}

fn sound_map_operation(map: *mut hp2_core::core_records::MapStorageRaw, key: i32, value: i32, op: i32) -> Result<i32, i32> {
    let mut context = SoundMapContext { map, key, value };
    let services = hp2_core::map_storage::Hp2MapServices {
        context: (&mut context as *mut SoundMapContext).cast(),
        leaf: sound_map_leaf,
        query: sound_map_query,
        allocator: hp2_core::array_storage::system_allocator(),
        tag: ptr::null(),
    };
    let slots = hp2_core::map_storage::Hp2MapSlots {
        hash: unsafe { ptr::addr_of_mut!((*map).hash) },
        count: unsafe { ptr::addr_of_mut!((*map).hash_count) },
    };
    unsafe { hp2_core::map_storage::run(slots, services, op) }
}

fn stream_map(sound: *mut USound) -> *mut hp2_core::core_records::MapStorageRaw {
    unsafe { ptr::addr_of_mut!((*sound).playing_streams).cast() }
}

fn sound_count_with_access(access: CoreAccess<'_>) -> RuntimeResult<usize> {
    let sound_class = access.native_class("Engine.Sound")?;
    let mut count = 0usize;
    for index in 0..access.object_count() {
        let object = access.object_at(index);
        if object.is_null() {
            continue;
        }
        let mut class = unsafe { ptr::addr_of!((*object).class).read_unaligned().cast::<UClass>() };
        while !class.is_null() {
            if class == sound_class {
                count += 1;
                break;
            }
            class = unsafe { ptr::addr_of!((*class).super_field).read_unaligned().cast::<UClass>() };
        }
    }
    Ok(count)
}

fn sound_fill_with_access(access: CoreAccess<'_>, out: &mut [usize]) -> RuntimeResult<()> {
    let sound_class = access.native_class("Engine.Sound")?;
    let mut written = 0usize;
    for index in 0..access.object_count() {
        let object = access.object_at(index);
        if object.is_null() {
            continue;
        }
        let mut class = unsafe { ptr::addr_of!((*object).class).read_unaligned().cast::<UClass>() };
        let mut is_sound = false;
        while !class.is_null() {
            if class == sound_class {
                is_sound = true;
                break;
            }
            class = unsafe { ptr::addr_of!((*class).super_field).read_unaligned().cast::<UClass>() };
        }
        if is_sound {
            let Some(slot) = out.get_mut(written) else {
                return Err(RuntimeError::new("Canonical USound list changed during snapshot"));
            };
            *slot = object as usize;
            written += 1;
        }
    }
    if written != out.len() {
        return Err(RuntimeError::new("Canonical USound snapshot size changed"));
    }
    Ok(())
}

impl CoreHost<'_> {
    pub(super) fn host_sound_info(&mut self, raw: usize) -> HostResult<SoundInfo> {
        let sound = sound_ptr(self, raw)?;
        let file_type = unsafe { ptr::addr_of!((*sound).file_type).read_unaligned() };
        let file_type = i32::from_ne_bytes(file_type);
        let access = self.access;
        let name_result = access.name_text(file_type);
        let name = core_result(self, name_result)?;
        let is_ogg = name
            .iter()
            .copied()
            .map(|unit| char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect::<String>()
            .eq_ignore_ascii_case("ogg");
        Ok(SoundInfo {
            core_flags: unsafe { ptr::addr_of!((*sound).core_flags).read_unaligned() },
            working_flags: unsafe { ptr::addr_of!((*sound).working_flags).read_unaligned() },
            is_ogg,
            raw_num_samples: unsafe { ptr::addr_of!((*sound).raw_num_samples).read_unaligned() },
            raw_bits_per_sample: unsafe { ptr::addr_of!((*sound).raw_bits_per_sample).read_unaligned() },
            raw_num_channels: unsafe { ptr::addr_of!((*sound).raw_num_channels).read_unaligned() },
            raw_sample_rate: unsafe { ptr::addr_of!((*sound).raw_sample_rate).read_unaligned() },
        })
    }

    pub(super) fn host_sound_data_len(&mut self, raw: usize) -> HostResult<i32> {
        let sound = sound_ptr(self, raw)?;
        let data = unsafe { ptr::addr_of!((*sound).data).cast::<crate::audio::SoundDataRaw>() };
        let array = unsafe { ptr::addr_of!((*data).lazy.array).read_unaligned() };
        Ok(array.array_num)
    }

    pub(super) fn host_sound_name(&mut self, raw: usize) -> HostResult<String> {
        let sound = sound_ptr(self, raw)?;
        let object = sound.cast::<UObject>();
        let name = i32::from_ne_bytes(unsafe { ptr::addr_of!((*object).name).read_unaligned() });
        let access = self.access;
        let units_result = access.name_text(name);
        let units = core_result(self, units_result)?;
        Ok(text_from_units(&units))
    }

    pub(super) fn host_sound_path_name(&mut self, raw: usize) -> HostResult<String> {
        let sound = sound_ptr(self, raw)?;
        let mut object = sound.cast::<UObject>();
        let mut chain = Vec::new();
        while !object.is_null() {
            let name = i32::from_ne_bytes(unsafe { ptr::addr_of!((*object).name).read_unaligned() });
            let access = self.access;
            let units_result = access.name_text(name);
            let units = core_result(self, units_result)?;
            chain.push(text_from_units(&units));
            object = unsafe { ptr::addr_of!((*object).outer).read_unaligned().cast() };
        }
        chain.reverse();
        Ok(chain.join("."))
    }

    pub(super) fn host_sound_filename(&mut self, raw: usize) -> HostResult<String> {
        let sound = sound_ptr(self, raw)?;
        let filename = unsafe { ptr::addr_of!((*sound).filename).read_unaligned() };
        core_result(self, fstring_text(filename))
    }

    pub(super) fn host_sound_set_working_flags(&mut self, raw: usize, flags: i32) -> HostResult<()> {
        let sound = sound_ptr(self, raw)?;
        unsafe { ptr::addr_of_mut!((*sound).working_flags).write_unaligned(flags); }
        Ok(())
    }

    pub(super) fn host_sound_set_handle(&mut self, raw: usize, handle: i32) -> HostResult<()> {
        let sound = sound_ptr(self, raw)?;
        unsafe { ptr::addr_of_mut!((*sound).handle).write_unaligned(handle); }
        Ok(())
    }

    pub(super) fn host_sound_set_duration(&mut self, raw: usize, duration: f32) -> HostResult<()> {
        let sound = sound_ptr(self, raw)?;
        unsafe { ptr::addr_of_mut!((*sound).duration).write_unaligned(duration); }
        Ok(())
    }

    pub(super) fn host_sound_add_ref(&mut self, raw: usize, delta: i32) -> HostResult<i32> {
        let sound = sound_ptr(self, raw)?;
        let ref_count = unsafe { ptr::addr_of_mut!((*sound).ref_count) };
        let count = unsafe { ref_count.read_unaligned().wrapping_add(delta) };
        unsafe { ref_count.write_unaligned(count); }
        Ok(count)
    }

    pub(super) fn host_sound_is_registered(&mut self, raw: usize) -> HostResult<bool> {
        let sound = sound_ptr(self, raw)?;
        Ok(unsafe { crate::audio::usound_is_registered(sound) })
    }

    pub(super) fn host_sound_wave_info(&mut self, raw: usize) -> HostResult<WaveInfo> {
        let sound = sound_ptr(self, raw)?;
        let data = unsafe { ptr::addr_of!((*sound).data).cast::<crate::audio::SoundDataRaw>() };
        let array = unsafe { ptr::addr_of!((*data).lazy.array).cast::<hp2_abi::render_scene::TArrayRaw>() };
        let mut wave = crate::audio::wave_mod_info_new();
        if !unsafe { crate::audio::read_wave_info(&mut wave, array) } {
            return Err(self.fail_with(RuntimeError::new("Invalid USound wave data")));
        }
        Ok(WaveInfo {
            sample_loops_num: wave.sample_loops_num,
            bits_per_sample: unsafe { wave.p_bits_per_sample.cast::<u16>().read_unaligned() as i32 },
            channels: unsafe { wave.p_channels.cast::<u16>().read_unaligned() as i32 },
            samples_per_sec: unsafe { wave.p_samples_per_sec.cast::<u32>().read_unaligned() as i32 },
            sample_data: wave.sample_data_start as usize,
            sample_data_size: wave.sample_data_size as i32,
        })
    }

    pub(super) fn host_sound_count(&mut self) -> HostResult<usize> {
        core_result(self, sound_count_with_access(self.access))
    }

    pub(super) fn host_sound_fill(&mut self, out: &mut [usize]) -> HostResult<()> {
        core_result(self, sound_fill_with_access(self.access, out))
    }

    pub(super) fn host_sound_load_data(&mut self, raw: usize) -> HostResult<()> {
        let sound = sound_ptr(self, raw)?;
        let data = unsafe { ptr::addr_of_mut!((*sound).data).cast::<crate::audio::SoundDataRaw>() };
        let slots = unsafe { lazy_sound_slots(data) };
        let access = self.access;
        let result = unsafe { hp2_core::lazy_array::lazy_array_load(access, slots) };
        core_result(self, result)
    }

    pub(super) fn host_sound_unload_data(&mut self, raw: usize) -> HostResult<()> {
        let sound = sound_ptr(self, raw)?;
        let data = unsafe { ptr::addr_of_mut!((*sound).data).cast::<crate::audio::SoundDataRaw>() };
        let slots = unsafe { lazy_sound_slots(data) };
        let access = self.access;
        let result = unsafe { hp2_core::lazy_array::lazy_array_unload(access, slots, None) };
        core_result(self, result)
    }

    pub(super) fn host_sound_add_stream_handle(&mut self, raw: usize, id: i32) -> HostResult<()> {
        let sound = sound_ptr(self, raw)?;
        let array = unsafe { ptr::addr_of_mut!((*sound).open_streams) };
        let result = unsafe {
            hp2_core::array_storage::add_zeroed(
                array_slots(array), 1, 4, ptr::null(), hp2_core::array_storage::system_allocator(),
            )
        };
        let index = result.map_err(|error| self.fail_with(core_array_error(error, "Unable to add sound stream handle")))?;
        let data = unsafe { ptr::addr_of!((*array).data).read_unaligned().cast::<i32>().add(index as usize) };
        unsafe { data.write_unaligned(id.wrapping_mul(2)); }
        Ok(())
    }

    pub(super) fn host_sound_get_handle(&mut self, raw: usize, id: i32) -> HostResult<i32> {
        let sound = sound_ptr(self, raw)?;
        if unsafe { ptr::addr_of!((*sound).core_flags).read_unaligned() }
            & hp2_audio::subsystem::SF_STREAMING
            == 0
        {
            return Ok(unsafe { ptr::addr_of!((*sound).handle).read_unaligned() });
        }
        let map = stream_map(sound);
        let index = sound_map_operation(
            map,
            id,
            0,
            hp2_core::map_storage::operation::FIND,
        )
        .map_err(|error| self.fail_with(core_array_error(error, "Unable to find sound stream")))?;
        if index < 0 {
            return Ok(0);
        }
        let mut context = SoundMapContext { map, key: id, value: 0 };
        let pair = unsafe { map_pair((&mut context as *mut SoundMapContext).cast(), index) };
        Ok(unsafe { ptr::addr_of!((*pair).value).read_unaligned() / 2 })
    }

    pub(super) fn host_sound_bind_stream(&mut self, raw: usize, id: i32) -> HostResult<()> {
        let sound = sound_ptr(self, raw)?;
        if unsafe { ptr::addr_of!((*sound).core_flags).read_unaligned() }
            & hp2_audio::subsystem::SF_STREAMING
            == 0
        {
            return Ok(());
        }
        let array = unsafe { ptr::addr_of_mut!((*sound).open_streams) };
        let header = unsafe { array.read_unaligned() };
        if header.num < 0 || (header.num > 0 && header.data.is_null()) {
            return Err(self.fail_with(RuntimeError::new("Invalid canonical USound open-stream array")));
        }
        for index in 0..header.num {
            let slot = unsafe { header.data.cast::<i32>().add(index as usize) };
            let value = unsafe { slot.read_unaligned() };
            if value & 1 == 0 {
                let result = sound_map_operation(
                    stream_map(sound),
                    id,
                    value,
                    hp2_core::map_storage::operation::SET,
                );
                if let Err(error) = result {
                    return Err(self.fail_with(core_array_error(error, "Unable to bind sound stream")));
                }
                unsafe { slot.write_unaligned(value | 1); }
                break;
            }
        }
        Ok(())
    }

    pub(super) fn host_sound_unbind_stream(&mut self, raw: usize, id: i32) -> HostResult<()> {
        let sound = sound_ptr(self, raw)?;
        let map = stream_map(sound);
        let index = sound_map_operation(map, id, 0, hp2_core::map_storage::operation::FIND)
            .map_err(|error| self.fail_with(core_array_error(error, "Unable to find sound stream")))?;
        let open = if index < 0 {
            0
        } else {
            let mut context = SoundMapContext { map, key: id, value: 0 };
            let pair = unsafe { map_pair((&mut context as *mut SoundMapContext).cast(), index) };
            unsafe { ptr::addr_of!((*pair).value).read_unaligned() }
        };
        let array = unsafe { ptr::addr_of_mut!((*sound).open_streams) };
        let header = unsafe { array.read_unaligned() };
        if header.num < 0 || (header.num > 0 && header.data.is_null()) {
            return Err(self.fail_with(RuntimeError::new("Invalid canonical USound open-stream array")));
        }
        for index in 0..header.num {
            let slot = unsafe { header.data.cast::<i32>().add(index as usize) };
            let value = unsafe { slot.read_unaligned() };
            if value & !1 == open {
                unsafe { slot.write_unaligned(value & !1); }
                break;
            }
        }
        sound_map_operation(map, id, 0, hp2_core::map_storage::operation::REMOVE)
            .map(|_| ())
            .map_err(|error| self.fail_with(core_array_error(error, "Unable to unbind sound stream")))
    }

    pub(super) fn host_sound_unbind_all_streams(&mut self, raw: usize) -> HostResult<()> {
        let sound = sound_ptr(self, raw)?;
        sound_map_operation(
            stream_map(sound),
            0,
            0,
            hp2_core::map_storage::operation::EMPTY,
        )
        .map(|_| ())
        .map_err(|error| self.fail_with(core_array_error(error, "Unable to unbind sound streams")))
    }

    pub(super) fn host_sound_playing_stream_count(&mut self, raw: usize) -> HostResult<usize> {
        let sound = sound_ptr(self, raw)?;
        let map = unsafe { stream_map(sound).read_unaligned() };
        if map.pairs.num < 0 || (map.pairs.num > 0 && map.pairs.data.is_null()) {
            return Err(self.fail_with(RuntimeError::new("Invalid canonical USound stream map")));
        }
        Ok(map.pairs.num as usize)
    }

    pub(super) fn host_sound_playing_stream_fill(
        &mut self,
        raw: usize,
        out: &mut [i32],
    ) -> HostResult<()> {
        let sound = sound_ptr(self, raw)?;
        let map = unsafe { stream_map(sound).read_unaligned() };
        if map.pairs.num < 0 || (map.pairs.num > 0 && map.pairs.data.is_null()) {
            return Err(self.fail_with(RuntimeError::new("Invalid canonical USound stream map")));
        }
        let count = map.pairs.num as usize;
        if out.len() < count {
            return Err(self.fail_with(RuntimeError::new("Sound stream output buffer is too small")));
        }
        for (index, destination) in out.iter_mut().take(count).enumerate() {
            let pair = unsafe { map.pairs.data.cast::<SoundStreamPair>().add(index) };
            *destination = unsafe { ptr::addr_of!((*pair).value).read_unaligned() / 2 };
        }
        Ok(())
    }

    pub(super) fn host_sound_delete_stream_handle(&mut self, raw: usize, handle: i32) -> HostResult<()> {
        let sound = sound_ptr(self, raw)?;
        let array = unsafe { ptr::addr_of_mut!((*sound).open_streams) };
        let header = unsafe { array.read_unaligned() };
        if header.num < 0 || (header.num > 0 && header.data.is_null()) {
            return Err(self.fail_with(RuntimeError::new("Invalid canonical USound open-stream array")));
        }
        let encoded = handle.wrapping_mul(2);
        for index in 0..header.num {
            let slot = unsafe { header.data.cast::<i32>().add(index as usize) };
            let value = unsafe { slot.read_unaligned() };
            if value & !1 == encoded {
                if value != encoded {
                    self.log(hp2_audio::host::LogKind::DevSound, "Deleteing a bound stream!")?;
                }
                let result = unsafe {
                    hp2_core::array_storage::remove(
                        array_slots(array), index, 1, 4, ptr::null(), hp2_core::array_storage::system_allocator(),
                    )
                };
                return result.map_err(|error| self.fail_with(core_array_error(error, "Unable to remove sound stream handle")));
            }
        }
        Ok(())
    }

    pub(super) fn host_sound_delete_all_stream_handles(&mut self, raw: usize) -> HostResult<()> {
        let sound = sound_ptr(self, raw)?;
        let array = unsafe { ptr::addr_of_mut!((*sound).open_streams) };
        let result = unsafe {
            hp2_core::array_storage::empty(
                array_slots(array), 0, 4, ptr::null(), hp2_core::array_storage::system_allocator(),
            )
        };
        result.map_err(|error| self.fail_with(core_array_error(error, "Unable to empty sound stream handles")))
    }

    pub(super) fn host_music_sound_new(&mut self, filename: &str) -> HostResult<usize> {
        let access = self.access;
        let class_result = access.native_class("Engine.Sound");
        let class = core_result(self, class_result)?;
        let file_type_result = access.intern_str("ogg", FindName::Add);
        let file_type = core_result(self, file_type_result)?;
        let mut filename_units: Vec<u32> = filename.chars().map(u32::from).collect();
        filename_units.push(0);
        let outer = access.transient_package();
        let result = unsafe { access.construct_object(class, outer, 0, 0, ptr::null_mut()) };
        let sound = core_result(self, result)?.cast::<USound>();
        let assign_result = unsafe {
            access.assign_fstring(ptr::addr_of_mut!((*sound).filename), &filename_units)
        };
        if let Err(error) = assign_result {
            let cleanup_result = unsafe { access.delete_object(sound.cast::<UObject>()) };
            let error = match cleanup_result {
                Ok(()) => error,
                Err(cleanup) => error.with_context(&format!("Sound cleanup failed: {cleanup}")),
            };
            return Err(self.fail_with(error));
        }
        unsafe {
            ptr::addr_of_mut!((*sound).file_type).write_unaligned(file_type.to_ne_bytes());
            let flags = hp2_audio::subsystem::SF_STREAMING | hp2_audio::subsystem::SF_MUSIC;
            ptr::addr_of_mut!((*sound).core_flags).write_unaligned(flags);
            ptr::addr_of_mut!((*sound).working_flags).write_unaligned(flags);
            ptr::addr_of_mut!((*sound).duration).write_unaligned(1.0);
        }
        Ok(sound as usize)
    }

}
