use hp2_abi::engine::generated::{FTextureInfo, UEngine, UFireTexture, ULinker, UObject};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_engine::EngineOwner;
use hp2_launcher::{
    boot::Hp2LauncherBootLeaf as BootLeaf,
    main_loop::Hp2LauncherMainLoopLeaf as LoopLeaf,
    reporters::{
        actor_slot_dump,
        fire_texture::{self, Hp2FireTextureServices, StepKind},
    },
};
use std::{
    ffi::{c_char, c_void},
    ptr,
};

static mut TARGET: *mut UFireTexture = ptr::null_mut();

pub(super) fn initialize_services(owner: &EngineOwner) -> RuntimeResult<()> {
    // The callbacks are direct reads of live canonical object fields. Generic
    // UObject path/class reads are supplied by RuntimeReporters.
    unsafe {
        fire_texture::publish_services(Hp2FireTextureServices {
            linker_filename_ansi,
            linker_index,
            texture_u_size,
            texture_v_size,
            active_spark_count,
            realtime,
            realtime_changed,
        });
    }
    owner.publish_actor_slot_dump_observer(actor_slot_dump_observer)
}

pub(super) fn detach(owner: &EngineOwner) -> RuntimeResult<()> {
    owner.clear_actor_slot_dump_observer()
}

unsafe extern "C" fn linker_filename_ansi(object: *mut c_void) -> *const c_char {
    if object.is_null() {
        return c"".as_ptr();
    }
    let linker = unsafe { ptr::read_unaligned(ptr::addr_of!((*object.cast::<UObject>())._linker)) }
        .cast::<ULinker>();
    if linker.is_null() {
        return c"".as_ptr();
    }
    let filename = unsafe { ptr::read_unaligned(ptr::addr_of!((*linker).filename)) };
    if filename.data.is_null() || filename.num <= 0 {
        return c"".as_ptr();
    }
    unsafe { hp2_core::ansi::to_ansi_string(filename.data.cast()) }.cast()
}

unsafe extern "C" fn linker_index(object: *mut c_void) -> i32 {
    if object.is_null() {
        -1
    } else {
        unsafe { ptr::read_unaligned(ptr::addr_of!((*object.cast::<UObject>())._linker_index)) }
    }
}

unsafe extern "C" fn texture_u_size(object: *mut c_void) -> i32 {
    if object.is_null() {
        0
    } else {
        unsafe {
            ptr::read_unaligned(ptr::addr_of!(
                (*object.cast::<hp2_abi::engine::generated::UTexture>())
                    .base
                    .u_size
            ))
        }
    }
}

unsafe extern "C" fn texture_v_size(object: *mut c_void) -> i32 {
    if object.is_null() {
        0
    } else {
        unsafe {
            ptr::read_unaligned(ptr::addr_of!(
                (*object.cast::<hp2_abi::engine::generated::UTexture>())
                    .base
                    .v_size
            ))
        }
    }
}

unsafe extern "C" fn active_spark_count(object: *mut c_void) -> i32 {
    if object.is_null() {
        0
    } else {
        unsafe { ptr::addr_of!((*object.cast::<UFireTexture>()).active_spark_num).read_unaligned() }
    }
}

unsafe extern "C" fn realtime(object: *mut c_void) -> i32 {
    i32::from(
        !object.is_null()
            && unsafe { (*object.cast::<hp2_abi::engine::generated::UTexture>()).b_realtime() },
    )
}

unsafe extern "C" fn realtime_changed(object: *mut c_void) -> i32 {
    i32::from(
        !object.is_null()
            && unsafe {
                (*object.cast::<hp2_abi::engine::generated::UTexture>()).b_realtime_changed()
            },
    )
}

pub(super) fn boot_leaf(leaf: BootLeaf, _arg: i32) -> RuntimeResult<Option<i32>> {
    use BootLeaf as L;
    let value = match leaf {
        L::ResolveFireTextureTarget => {
            let loaded = crate::reporters::with_owner(ptr::null_mut(), |owner| {
                let access = owner.access();
                let class = access.native_class("Fire.FireTexture")?.cast();
                let path: Vec<u32> = "HPParticle.hp_fx.Fire1"
                    .chars()
                    .map(u32::from)
                    .chain([0])
                    .collect();
                unsafe { access.static_load_object(class, ptr::null_mut(), &path, 0x0001) }
                    .map(|object| object.cast::<UFireTexture>())
            });
            crate::reporters::check_callback_error()?;
            if loaded.is_null() {
                return Err(RuntimeError::new(
                    "Could not load HPParticle.hp_fx.Fire1 for HP2_FIRE_TEXTURE_TRACE",
                ));
            }
            unsafe {
                TARGET = loaded;
            }
            Some(1)
        }
        L::CheckFireTextureProvenance => {
            let target = unsafe { TARGET };
            if target.is_null() {
                return Err(RuntimeError::new(
                    "Fire texture provenance checked before target resolution",
                ));
            }
            let (index, path) = crate::reporters::with_owner((-1, Vec::new()), |owner| {
                let access = owner.access();
                let mut path = Vec::new();
                unsafe {
                    access.write_object_path_text(target.cast::<UObject>(), &mut path)?;
                }
                let index = unsafe {
                    ptr::read_unaligned(ptr::addr_of!((*target.cast::<UObject>())._linker_index))
                };
                Ok((index, path))
            });
            crate::reporters::check_callback_error()?;
            let mut text = String::new();
            for unit in path.iter().take_while(|&&unit| unit != 0) {
                text.push(char::from_u32(*unit).ok_or_else(|| {
                    RuntimeError::new("Fire texture path contains invalid UTF-32")
                })?);
            }
            Some(i32::from(
                index != 886 || !text.eq_ignore_ascii_case("HPParticle.hp_fx.Fire1"),
            ))
        }
        L::SetFireTextureTarget => {
            let target = unsafe { TARGET };
            if target.is_null() {
                return Err(RuntimeError::new("Fire texture target was not resolved"));
            }
            unsafe {
                fire_texture::set_target(target.cast());
            }
            Some(0)
        }
        L::RunFireTextureProbe => {
            let target = unsafe { TARGET };
            if target.is_null() {
                return Err(RuntimeError::new(
                    "Fire texture probe target was not resolved",
                ));
            }
            unsafe {
                run_probe(target)?;
            }
            Some(0)
        }
        L::ErrorFireTextureLoadFailed => {
            return Err(RuntimeError::new(
                "Could not load HPParticle.hp_fx.Fire1 for HP2_FIRE_TEXTURE_TRACE",
            ));
        }
        L::ErrorFireTextureProvenance => {
            return Err(RuntimeError::new(
                "HP2_FIRE_TEXTURE_TRACE target provenance is not HPParticle.hp_fx.Fire1 export 886",
            ));
        }
        _ => None,
    };
    Ok(value)
}

unsafe fn run_probe(target: *mut UFireTexture) -> RuntimeResult<()> {
    if unsafe { fire_texture::probe_begin(target.cast()) } == 0 {
        return Ok(());
    }
    let mut info: FTextureInfo = unsafe { std::mem::zeroed() };
    let steps = [
        (
            StepKind::Lock,
            hp2_engine::FireTextureProbeStep::Lock,
            b"lock".as_slice(),
        ),
        (
            StepKind::Update,
            hp2_engine::FireTextureProbeStep::Update,
            b"update".as_slice(),
        ),
        (
            StepKind::Tick,
            hp2_engine::FireTextureProbeStep::Tick,
            b"tick".as_slice(),
        ),
        (
            StepKind::ConstantTimeTick,
            hp2_engine::FireTextureProbeStep::ConstantTimeTick,
            b"constant_time_tick".as_slice(),
        ),
    ];
    for (kind, operation, phase) in steps {
        unsafe {
            fire_texture::step_begin(kind as i32);
        }
        probe_marker(phase, true);
        let changed = crate::reporters::with_owner(false, |owner| unsafe {
            hp2_engine::fire_texture_probe_step(owner.access(), target, &mut info, operation)
        });
        crate::reporters::check_callback_error()?;
        probe_marker(phase, false);
        unsafe {
            fire_texture::step_end(kind as i32, i32::from(changed));
        }
    }
    unsafe {
        fire_texture::probe_end();
    }
    crate::reporters::with_owner((), |owner| unsafe {
        hp2_engine::fire_texture_probe_step(
            owner.access(),
            target,
            &mut info,
            hp2_engine::FireTextureProbeStep::Unlock,
        )
        .map(|_| ())
    });
    crate::reporters::check_callback_error()?;
    Ok(())
}

fn probe_marker(phase: &[u8], beginning: bool) {
    use std::io::Write;
    let mut stderr = std::io::stderr().lock();
    let _ = stderr.write_all(b"<HP2_FIRE_TEXTURE_TRACE> phase=");
    let _ = stderr.write_all(phase);
    let _ = stderr.write_all(if beginning { b"_begin\n" } else { b"_end\n" });
}

pub(super) fn loop_leaf(leaf: LoopLeaf, engine: *mut UEngine) -> RuntimeResult<Option<i32>> {
    use LoopLeaf as L;
    crate::reporters::check_callback_error()?;
    let result = match leaf {
        L::ActorSlotDumpEnabled => Some(i32::from(actor_slot_dump::enabled())),
        L::CapturePostStartup => {
            unsafe {
                actor_slot_dump::capture_post_startup_before_first_tick(engine.cast());
            }
            Some(0)
        }
        L::WriteActorSlotDump => Some(unsafe {
            actor_slot_dump::write_if_ready_with(
                engine.cast(),
                crate::reporters::check_callback_error,
            )?
        }),
        _ => None,
    };
    crate::reporters::check_callback_error()?;
    Ok(result)
}

pub(super) fn fire_observer(
    _access: CoreAccess<'_>,
    event: hp2_engine::FireTraceEvent,
) -> RuntimeResult<()> {
    unsafe {
        match event {
            hp2_engine::FireTraceEvent::InitTables {
                before,
                after,
                fingerprint,
            } => fire_texture::init_tables(before, after, fingerprint),
            hp2_engine::FireTraceEvent::SpeedRand { index, value } => {
                fire_texture::speed_rand(index, value)
            }
            hp2_engine::FireTraceEvent::BurnWrite {
                texture,
                spark,
                offset,
                before,
                value,
            } => fire_texture::burn_write(texture, spark, offset, before, value),
        }
    }
    Ok(())
}
pub(super) fn actor_slot_dump_observer(
    _access: CoreAccess<'_>,
    event: hp2_engine::ActorSlotDumpEvent,
) -> RuntimeResult<()> {
    unsafe {
        match event {
            hp2_engine::ActorSlotDumpEvent::PostDeserialize { level } => {
                actor_slot_dump::capture_post_deserialize(level)
            }
            hp2_engine::ActorSlotDumpEvent::BeginRawTable { level } => {
                actor_slot_dump::begin_raw_level_actor_table(level)
            }
            hp2_engine::ActorSlotDumpEvent::RawActor {
                level,
                slot,
                actor,
                reference_offset_before,
                reference_offset_after,
                compact_package_index,
                compact_package_index_captured,
                compact_package_index_supported,
            } => actor_slot_dump::capture_raw_level_actor(
                level,
                slot,
                actor,
                reference_offset_before,
                reference_offset_after,
                compact_package_index,
                i32::from(compact_package_index_captured),
                i32::from(compact_package_index_supported),
            ),
            hp2_engine::ActorSlotDumpEvent::EndRawTable { level } => {
                actor_slot_dump::end_raw_level_actor_table(level)
            }
        }
    }
    crate::reporters::check_callback_error()
}
