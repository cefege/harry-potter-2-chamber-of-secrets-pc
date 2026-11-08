//! Canonical Core leaves for the shared StatLog policy.
//!
//! CoreAccess is scoped to synchronous service calls. Reentrant callbacks restore
//! the previous scope and retain the first Core error until the policy returns.
use crate::params::TChar;
use hp2_abi::engine::generated::{
    AActor, AGameInfo, ALevelInfo, AMutator, APlayerPawn, APlayerReplicationInfo, AStatLog,
    AStatLogEventSpecial2Params, AStatLogEventSpecialParams, AStatLogFile, TArrayRaw, UClass,
    UFunction, ULevel, UObject,
};
use hp2_core::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_class_specs::{NativeClassSpec, NativeRecordKind, StartupProperty, StartupPropertyKind},
    name_owner::FindName,
    object_owner::ClassOps,
};
use std::{
    alloc::Layout,
    cell::{Cell, RefCell},
    ffi::{CStr, c_void},
    ptr,
    sync::Once,
};

const OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const RF_PUBLIC: u32 = 4;
const CPF_EDIT: u32 = 1;
const CPF_CONFIG: u32 = 0x0000_4000;
const CPF_GLOBAL_CONFIG: u32 = 0x0004_0000;

const fn prop(
    name: &'static str,
    kind: StartupPropertyKind,
    property_flags: u32,
    offset: usize,
) -> StartupProperty {
    StartupProperty {
        name,
        kind,
        category: "",
        property_flags,
        object_flags: RF_PUBLIC,
        inner_name: None,
        offset,
        array_dim: 1,
    }
}

const fn bit_prop(name: &'static str, bit: usize, property_flags: u32) -> StartupProperty {
    prop(
        name,
        StartupPropertyKind::Bool {
            bit_mask: 1u32 << (bit % 32),
        },
        property_flags,
        bit / 32 * 4,
    )
}

const MUTATOR_PROPERTIES: &[StartupProperty] = &[
    prop(
        "NextMutator",
        StartupPropertyKind::Object {
            class: "Engine.Mutator",
        },
        0,
        std::mem::offset_of!(AMutator, next_mutator),
    ),
    prop(
        "NextDamageMutator",
        StartupPropertyKind::Object {
            class: "Engine.Mutator",
        },
        0,
        std::mem::offset_of!(AMutator, next_damage_mutator),
    ),
    prop(
        "NextMessageMutator",
        StartupPropertyKind::Object {
            class: "Engine.Mutator",
        },
        0,
        std::mem::offset_of!(AMutator, next_message_mutator),
    ),
    prop(
        "NextHUDMutator",
        StartupPropertyKind::Object {
            class: "Engine.Mutator",
        },
        0,
        std::mem::offset_of!(AMutator, next_hud_mutator),
    ),
    bit_prop("bHUDMutator", AMutator::B_HUD_MUTATOR_BIT, 0),
    prop(
        "DefaultWeapon",
        StartupPropertyKind::Class {
            meta_class: "Engine.Weapon",
        },
        0,
        std::mem::offset_of!(AMutator, default_weapon),
    ),
];

const STAT_LOG_CONFIG: u32 = CPF_EDIT | CPF_CONFIG | CPF_GLOBAL_CONFIG;
const STAT_LOG_PROPERTIES: &[StartupProperty] = &[
    prop(
        "Context",
        StartupPropertyKind::Int,
        0,
        std::mem::offset_of!(AStatLog, context),
    ),
    bit_prop("bWorld", AStatLog::B_WORLD_BIT, 0),
    prop(
        "TimeStamp",
        StartupPropertyKind::Float,
        0,
        std::mem::offset_of!(AStatLog, time_stamp),
    ),
    prop(
        "LocalStandard",
        StartupPropertyKind::String,
        CPF_EDIT,
        std::mem::offset_of!(AStatLog, local_standard),
    ),
    prop(
        "WorldStandard",
        StartupPropertyKind::String,
        CPF_EDIT,
        std::mem::offset_of!(AStatLog, world_standard),
    ),
    prop(
        "LogVersion",
        StartupPropertyKind::String,
        CPF_EDIT,
        std::mem::offset_of!(AStatLog, log_version),
    ),
    prop(
        "LogInfoURL",
        StartupPropertyKind::String,
        CPF_EDIT,
        std::mem::offset_of!(AStatLog, log_info_url),
    ),
    prop(
        "GameName",
        StartupPropertyKind::String,
        CPF_EDIT,
        std::mem::offset_of!(AStatLog, game_name),
    ),
    prop(
        "GameCreator",
        StartupPropertyKind::String,
        CPF_EDIT,
        std::mem::offset_of!(AStatLog, game_creator),
    ),
    prop(
        "GameCreatorURL",
        StartupPropertyKind::String,
        CPF_EDIT,
        std::mem::offset_of!(AStatLog, game_creator_url),
    ),
    prop(
        "DecoderRingURL",
        StartupPropertyKind::String,
        CPF_EDIT,
        std::mem::offset_of!(AStatLog, decoder_ring_url),
    ),
    prop(
        "LocalBatcherURL",
        StartupPropertyKind::String,
        STAT_LOG_CONFIG,
        std::mem::offset_of!(AStatLog, local_batcher_url),
    ),
    prop(
        "LocalBatcherParams",
        StartupPropertyKind::String,
        STAT_LOG_CONFIG,
        std::mem::offset_of!(AStatLog, local_batcher_params),
    ),
    prop(
        "LocalStatsURL",
        StartupPropertyKind::String,
        STAT_LOG_CONFIG,
        std::mem::offset_of!(AStatLog, local_stats_url),
    ),
    prop(
        "WorldBatcherURL",
        StartupPropertyKind::String,
        STAT_LOG_CONFIG,
        std::mem::offset_of!(AStatLog, world_batcher_url),
    ),
    prop(
        "WorldBatcherParams",
        StartupPropertyKind::String,
        STAT_LOG_CONFIG,
        std::mem::offset_of!(AStatLog, world_batcher_params),
    ),
    prop(
        "WorldStatsURL",
        StartupPropertyKind::String,
        STAT_LOG_CONFIG,
        std::mem::offset_of!(AStatLog, world_stats_url),
    ),
    prop(
        "LocalLogDir",
        StartupPropertyKind::String,
        STAT_LOG_CONFIG,
        std::mem::offset_of!(AStatLog, local_log_dir),
    ),
    prop(
        "WorldLogDir",
        StartupPropertyKind::String,
        STAT_LOG_CONFIG,
        std::mem::offset_of!(AStatLog, world_log_dir),
    ),
    bit_prop(
        "bWorldBatcherError",
        AStatLog::B_WORLD_BATCHER_ERROR_BIT,
        CPF_CONFIG | CPF_GLOBAL_CONFIG,
    ),
];

const STAT_LOG_FILE_PROPERTIES: &[StartupProperty] = &[
    bit_prop("bWatermark", AStatLogFile::B_WATERMARK_BIT, 0),
    prop(
        "LogAr",
        StartupPropertyKind::Int,
        0,
        std::mem::offset_of!(AStatLogFile, log_ar),
    ),
    prop(
        "StatLogFile",
        StartupPropertyKind::String,
        0,
        std::mem::offset_of!(AStatLogFile, stat_log_file),
    ),
    prop(
        "StatLogFinal",
        StartupPropertyKind::String,
        0,
        std::mem::offset_of!(AStatLogFile, stat_log_final),
    ),
];

static SIGCHLD_INSTALL: Once = Once::new();

struct Context {
    access: CoreAccess<'static>,
    error: RefCell<Option<RuntimeError>>,
}
thread_local! { static ACTIVE: Cell<*mut Context> = const { Cell::new(ptr::null_mut()) }; }
struct Scope(*mut Context);
impl Drop for Scope {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(self.0));
    }
}

fn with_context<T>(
    access: CoreAccess<'_>,
    body: impl FnOnce() -> Result<T, i32>,
) -> RuntimeResult<T> {
    let mut context = Context {
        access: unsafe { std::mem::transmute::<CoreAccess<'_>, CoreAccess<'static>>(access) },
        error: RefCell::new(None),
    };
    let previous = ACTIVE.with(|active| active.replace(&raw mut context));
    let _scope = Scope(previous);
    match body() {
        Ok(value) => match context.error.into_inner() {
            Some(error) => Err(error),
            None => Ok(value),
        },
        Err(status) => Err(context.error.into_inner().unwrap_or_else(|| {
            RuntimeError::new("Canonical StatLog policy failed").with_context(&status.to_string())
        })),
    }
}
fn context() -> Option<&'static Context> {
    ACTIVE.with(|active| {
        let raw = active.get();
        (!raw.is_null()).then(|| unsafe { &*raw })
    })
}
fn remember(ctx: &Context, error: RuntimeError) -> i32 {
    let mut first = ctx.error.borrow_mut();
    if first.is_none() {
        *first = Some(error);
    }
    1
}

/// Scope used by the IO/MD5 owner; callback failures take precedence over a policy status.
pub(super) fn boundary<T: Copy>(
    fallback: T,
    operation: impl FnOnce(&CoreAccess<'_>) -> RuntimeResult<T>,
) -> T {
    let Some(ctx) = context() else {
        return fallback;
    };
    match operation(&ctx.access) {
        Ok(value) => value,
        Err(error) => {
            remember(ctx, error);
            fallback
        }
    }
}

unsafe extern "C" fn actor_level(actor: *mut c_void) -> *mut c_void {
    if actor.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        ptr::addr_of_mut!((*actor.cast::<AActor>()).level)
            .read_unaligned()
            .cast()
    }
}
unsafe extern "C" fn level_game(level: *mut c_void) -> *mut c_void {
    if level.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        ptr::addr_of_mut!((*level.cast::<ALevelInfo>()).game)
            .read_unaligned()
            .cast()
    }
}
unsafe extern "C" fn level_not_dedicated(level: *mut c_void) -> i32 {
    if level.is_null() {
        return 0;
    }
    i32::from(unsafe {
        ptr::addr_of!((*level.cast::<ALevelInfo>()).net_mode).read_unaligned() != 1
    })
}
unsafe extern "C" fn game_external_batcher(game: *mut c_void) -> i32 {
    if game.is_null() {
        return 0;
    }
    i32::from(unsafe { (*game.cast::<AGameInfo>()).b_external_batcher() })
}
unsafe extern "C" fn game_local_log_file_name(game: *mut c_void) -> *mut c_void {
    if game.is_null() {
        return ptr::null_mut();
    }
    unsafe { ptr::addr_of_mut!((*game.cast::<AGameInfo>()).local_log_file_name).cast() }
}
unsafe extern "C" fn fstr_chars(value: *mut c_void) -> *const TChar {
    if value.is_null() {
        return ptr::null();
    }
    unsafe { hp2_core::native_text::fstring_text(value.cast()).cast() }
}
unsafe extern "C" fn fstr_len(value: *mut c_void) -> i32 {
    if value.is_null() {
        return 0;
    }
    unsafe {
        ptr::addr_of!((*value.cast::<TArrayRaw>()).num)
            .read_unaligned()
            .saturating_sub(1)
            .max(0)
    }
}
unsafe extern "C" fn fstr_assign(value: *mut c_void, text: *const TChar) {
    let _ = boundary((), |access| {
        if value.is_null() || text.is_null() {
            return Err(RuntimeError::new("Invalid StatLog FString assignment"));
        }
        let length = unsafe { hp2_core::unix_text::tchar_length(text.cast()) };
        let src = unsafe { std::slice::from_raw_parts(text.cast::<u32>(), length + 1) };
        unsafe { access.assign_fstring(value.cast(), src) }
    });
}
unsafe extern "C" fn fstr_append(value: *mut c_void, text: *const TChar) {
    let _ = boundary((), |access| {
        if value.is_null() || text.is_null() {
            return Err(RuntimeError::new("Invalid StatLog FString append"));
        }
        unsafe { access.append_vm_fstring_result(value.cast(), terminated_tchar(text)) }
    });
}
unsafe fn terminated_tchar<'a>(text: *const TChar) -> &'a [TChar] {
    let n = unsafe { hp2_core::unix_text::tchar_length(text.cast()) };
    unsafe { std::slice::from_raw_parts(text, n + 1) }
}
unsafe extern "C" fn local_batcher_url(this: *mut c_void) -> *mut c_void {
    if this.is_null() {
        ptr::null_mut()
    } else {
        unsafe { ptr::addr_of_mut!((*this.cast::<AStatLog>()).local_batcher_url).cast() }
    }
}
unsafe extern "C" fn local_log_dir(this: *mut c_void) -> *mut c_void {
    if this.is_null() {
        ptr::null_mut()
    } else {
        unsafe { ptr::addr_of_mut!((*this.cast::<AStatLog>()).local_log_dir).cast() }
    }
}
unsafe extern "C" fn world_batcher_url(this: *mut c_void) -> *mut c_void {
    if this.is_null() {
        ptr::null_mut()
    } else {
        unsafe { ptr::addr_of_mut!((*this.cast::<AStatLog>()).world_batcher_url).cast() }
    }
}
unsafe extern "C" fn world_batcher_params(this: *mut c_void) -> *mut c_void {
    if this.is_null() {
        ptr::null_mut()
    } else {
        unsafe { ptr::addr_of_mut!((*this.cast::<AStatLog>()).world_batcher_params).cast() }
    }
}
unsafe extern "C" fn class_default(this: *mut c_void) -> *mut c_void {
    boundary(ptr::null_mut(), |access| {
        if this.is_null() {
            return Err(RuntimeError::new("Null StatLog class instance"));
        }
        let class = unsafe {
            ptr::addr_of!((*this.cast::<UObject>()).class)
                .read_unaligned()
                .cast::<UClass>()
        };
        unsafe { access.default_object(class) }.map(|p| p.cast())
    })
}
unsafe extern "C" fn static_default() -> *mut c_void {
    boundary(ptr::null_mut(), |access| {
        let class = access.native_class("Engine.StatLog")?;
        unsafe { access.default_object(class) }.map(|p| p.cast())
    })
}
unsafe extern "C" fn get_world_error(this: *mut c_void) -> i32 {
    if this.is_null() {
        0
    } else {
        i32::from(unsafe { (*this.cast::<AStatLog>()).b_world_batcher_error() })
    }
}
unsafe extern "C" fn set_world_error(this: *mut c_void, value: i32) {
    if !this.is_null() {
        unsafe { (*this.cast::<AStatLog>()).set_b_world_batcher_error(value != 0) }
    }
}
unsafe extern "C" fn save_config(this: *mut c_void) {
    let _ = boundary((), |access| {
        if this.is_null() {
            return Err(RuntimeError::new("Null StatLog SaveConfig object"));
        }
        unsafe { access.save_config(this.cast(), 0x4000, None) }
    });
}
unsafe extern "C" fn get_world(this: *mut c_void) -> i32 {
    if this.is_null() {
        0
    } else {
        i32::from(unsafe { (*this.cast::<AStatLog>()).b_world() })
    }
}
unsafe extern "C" fn file_name(this: *mut c_void) -> *mut c_void {
    if this.is_null() {
        ptr::null_mut()
    } else {
        unsafe { ptr::addr_of_mut!((*this.cast::<AStatLogFile>()).stat_log_file).cast() }
    }
}
unsafe extern "C" fn file_final(this: *mut c_void) -> *mut c_void {
    if this.is_null() {
        ptr::null_mut()
    } else {
        unsafe { ptr::addr_of_mut!((*this.cast::<AStatLogFile>()).stat_log_final).cast() }
    }
}
unsafe extern "C" fn player_name(pawn: *mut c_void) -> *mut c_void {
    if pawn.is_null() {
        return ptr::null_mut();
    }
    let pri = unsafe {
        ptr::addr_of!((*pawn.cast::<APlayerPawn>()).base.player_replication_info).read_unaligned()
    };
    if pri.is_null() {
        ptr::null_mut()
    } else {
        unsafe { ptr::addr_of_mut!((*pri.cast::<APlayerReplicationInfo>()).player_name).cast() }
    }
}
unsafe extern "C" fn player_secret(pawn: *mut c_void) -> *mut c_void {
    if pawn.is_null() {
        ptr::null_mut()
    } else {
        unsafe { ptr::addr_of_mut!((*pawn.cast::<APlayerPawn>()).ng_world_secret).cast() }
    }
}

fn find_function(
    access: CoreAccess<'_>,
    object: *mut c_void,
    name: &str,
) -> RuntimeResult<*mut UFunction> {
    if object.is_null() {
        return Err(RuntimeError::new("Null StatLog event target"));
    }
    let klass = unsafe {
        ptr::addr_of!((*object.cast::<UObject>()).class)
            .read_unaligned()
            .cast::<UClass>()
    };
    let name = access.intern_str(name, FindName::Find)?;
    let kind = access.native_class("Core.Function")?;
    let result = unsafe { access.find_field(klass.cast(), name, kind) };
    if result.is_null() {
        Err(RuntimeError::new("StatLog event function not found"))
    } else {
        Ok(result.cast())
    }
}
unsafe extern "C" fn event_special_class(this: *mut c_void, id: *const TChar, target: *mut c_void) {
    let _ = boundary((), |access| {
        if this.is_null() || id.is_null() || target.is_null() {
            return Err(RuntimeError::new("Invalid StatLog special event"));
        }
        let target_class =
            unsafe { ptr::addr_of!((*target.cast::<UObject>()).class).read_unaligned() };
        let mut target_name = unsafe { access.object_full_name(target_class.cast())? };
        if target_name.last() != Some(&0) {
            target_name.push(0)
        }
        let mut params = AStatLogEventSpecialParams {
            special_id: TArrayRaw {
                data: ptr::null_mut(),
                num: 0,
                max: 0,
            },
            special_param: TArrayRaw {
                data: ptr::null_mut(),
                num: 0,
                max: 0,
            },
        };
        let result = (|| {
            let id_text = unsafe {
                std::slice::from_raw_parts(
                    id.cast::<u32>(),
                    hp2_core::unix_text::tchar_length(id.cast()) + 1,
                )
            };
            unsafe {
                access.assign_fstring(&mut params.special_id, id_text)?;
                access.assign_fstring(&mut params.special_param, &target_name)?
            }
            let function = find_function(*access, this, "LogGameSpecial")?;
            unsafe {
                access.execute_event(
                    this.cast(),
                    function,
                    ptr::from_mut(&mut params).cast(),
                    ptr::null_mut(),
                )
            }
        })();
        let first = unsafe { destroy_string(*access, &mut params.special_id) };
        let second = unsafe { destroy_string(*access, &mut params.special_param) };
        result.and(first).and(second)
    });
}
unsafe extern "C" fn event_special2(
    this: *mut c_void,
    id: *const TChar,
    p1: *const TChar,
    p2: *const TChar,
) {
    let _ = boundary((), |access| {
        if this.is_null() || id.is_null() || p1.is_null() || p2.is_null() {
            return Err(RuntimeError::new("Invalid StatLog special event"));
        }
        let mut params = AStatLogEventSpecial2Params {
            special_id: TArrayRaw {
                data: ptr::null_mut(),
                num: 0,
                max: 0,
            },
            special_param: TArrayRaw {
                data: ptr::null_mut(),
                num: 0,
                max: 0,
            },
            special_param2: TArrayRaw {
                data: ptr::null_mut(),
                num: 0,
                max: 0,
            },
        };
        let result = (|| {
            for (destination, source) in [
                (&mut params.special_id, id),
                (&mut params.special_param, p1),
                (&mut params.special_param2, p2),
            ] {
                let text = unsafe {
                    std::slice::from_raw_parts(
                        source.cast::<u32>(),
                        hp2_core::unix_text::tchar_length(source.cast()) + 1,
                    )
                };
                unsafe { access.assign_fstring(destination, text)? }
            }
            let function = find_function(*access, this, "LogGameSpecial2")?;
            unsafe {
                access.execute_event(
                    this.cast(),
                    function,
                    ptr::from_mut(&mut params).cast(),
                    ptr::null_mut(),
                )
            }
        })();
        let first = unsafe { destroy_string(*access, &mut params.special_id) };
        let second = unsafe { destroy_string(*access, &mut params.special_param) };
        let third = unsafe { destroy_string(*access, &mut params.special_param2) };
        result.and(first).and(second).and(third)
    });
}
unsafe extern "C" fn iterate_packages(
    callback: unsafe extern "C" fn(*mut c_void, *mut c_void),
    user: *mut c_void,
) {
    let _ = boundary((), |access| {
        let class_class = access.native_class("Core.Class")?;
        let package_class = access.native_class("Core.Package")?;
        for index in 0..access.object_count() {
            let object = access.object_at(index);
            if object.is_null()
                || !unsafe { hp2_core::object_model::is_a(object.cast(), class_class.cast()) }
            {
                continue;
            }
            let outer = unsafe { ptr::addr_of!((*object).outer).read_unaligned() };
            if outer.is_null()
                || !unsafe { hp2_core::object_model::is_a(outer.cast(), package_class.cast()) }
            {
                return Err(RuntimeError::new("StatLog class outer is not a package"));
            }
            unsafe { callback(user, outer.cast()) };
        }
        Ok(())
    });
}
thread_local! {
    static FULL_NAME: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
}
unsafe extern "C" fn package_full_name(package: *mut c_void) -> *const TChar {
    boundary(ptr::null(), |access| {
        if package.is_null() {
            return Err(RuntimeError::new("Null StatLog package"));
        }
        let mut name = unsafe { access.object_full_name(package.cast())? };
        if name.last() != Some(&0) {
            name.push(0);
        }
        FULL_NAME.with(|slot| {
            let mut buffer = slot.borrow_mut();
            *buffer = name;
            Ok(buffer.as_ptr().cast())
        })
    })
}
static DLL_EXT_MACOS: [TChar; 7] = [46, 100, 121, 108, 105, 98, 0];
static DLL_EXT_OTHER: [TChar; 4] = [46, 115, 111, 0];
unsafe extern "C" fn dll_ext() -> *const TChar {
    if cfg!(target_os = "macos") {
        DLL_EXT_MACOS.as_ptr()
    } else {
        DLL_EXT_OTHER.as_ptr()
    }
}
fn emit_logf(access: CoreAccess<'_>, text: &[u32]) -> RuntimeResult<()> {
    let category = access.intern_str("Log", FindName::Find)?;
    if access.name_suppressed(category) {
        return Ok(());
    }
    access.emit_log(category, if text.len() < 4096 { text } else { &[] })
}
unsafe extern "C" fn debug_text(text: *const TChar) {
    emit_stat_log_text(text);
}
unsafe extern "C" fn warn_text(text: *const TChar) {
    emit_stat_log_text(text);
}
fn emit_stat_log_text(text: *const TChar) {
    let _ = boundary((), |access| {
        if text.is_null() {
            return Err(RuntimeError::new("Null StatLog diagnostic text"));
        }
        let len = unsafe { hp2_core::unix_text::tchar_length(text.cast()) };
        let units = unsafe { std::slice::from_raw_parts(text.cast::<u32>(), len) };
        emit_logf(*access, units)
    });
}
unsafe extern "C" fn requesting_exit() {
    hp2_core::unix_signal::set_exit_requested(true)
}
unsafe extern "C" fn proc_create(
    url: *const TChar,
    params: *const TChar,
    _show: i32,
) -> *mut c_void {
    boundary(ptr::null_mut(), |access| {
        let mut message = b"Create Proc: "
            .iter()
            .map(|byte| u32::from(*byte))
            .collect::<Vec<_>>();
        for (index, text) in [url, params].into_iter().enumerate() {
            if index != 0 {
                message.push(32);
            }
            if !text.is_null() {
                let len = unsafe { hp2_core::unix_text::tchar_length(text.cast()) };
                message.extend(unsafe { std::slice::from_raw_parts(text.cast::<u32>(), len) });
            }
        }
        emit_logf(*access, &message)?;
        let services = hp2_core::unix_text::Hp2UnixTextServices {
            opaque: ptr::null_mut(),
            allocate: unix_allocate,
            fatal: unix_fatal,
        };
        let command = unsafe {
            hp2_core::unix_process::build_proc_command(&services, url.cast(), params.cast())
        }
        .map_err(|status| {
            RuntimeError::new("StatLog process command failed").with_context(&status.to_string())
        })?;
        // Canonical Core has no separate appPlatformInit callback; install the same
        // process reaper exactly once at the first platform process-launch seam.
        SIGCHLD_INSTALL.call_once(hp2_core::unix_process::install_sigchld_handler);
        let pid = unsafe { hp2_core::unix_process::create_proc(command) };
        Ok(if pid < 0 {
            ptr::null_mut()
        } else {
            pid as usize as *mut c_void
        })
    })
}
unsafe extern "C" fn proc_return_code(handle: *mut c_void, rc: *mut i32) -> i32 {
    boundary(0, |_| {
        if rc.is_null() {
            return Err(RuntimeError::new("Null StatLog process return code"));
        }
        let Some(code) = hp2_core::unix_process::get_proc_return_code(handle as usize as u64)
        else {
            return Ok(0);
        };
        unsafe { rc.write(code) };
        Ok(1)
    })
}
unsafe extern "C" fn launch_url(_url: *const TChar) {
    hp2_core::unix_process::launch_url();
}
unsafe extern "C" fn actor_level_url_map(actor: *mut c_void) -> *mut c_void {
    if actor.is_null() {
        return ptr::null_mut();
    }
    let level = unsafe { ptr::addr_of!((*actor.cast::<AActor>()).x_level).read_unaligned() };
    if level.is_null() {
        return ptr::null_mut();
    }
    unsafe { ptr::addr_of_mut!((*level.cast::<ULevel>()).base.url.map).cast() }
}
unsafe extern "C" fn raise(status: i32, expr: *const c_void, line: i32) -> i32 {
    if status == 0 {
        return 0;
    }
    let Some(ctx) = context() else {
        return 1;
    };
    if status == 1 && hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.checks == 0 {
        return 0;
    }

    let error = match status {
        1 if expr.is_null() => RuntimeError::new("Null StatLog assertion expression"),
        1 => {
            let expression = unsafe { CStr::from_ptr(expr.cast()) }.to_string_lossy();
            RuntimeError::new(&format!(
                "Assertion failed: {expression} [File:HarryPotter2/Unreal/Engine/Src/AStatLog.cpp] [Line: {line}]"
            ))
        }
        3 => RuntimeError::new("UTF-16LE text exceeds the 32-bit MD5 contract"),
        4 => RuntimeError::new("Stat log entry exceeds the 32-bit archive contract"),
        5 => RuntimeError::new("Stat log native panicked"),
        _ => RuntimeError::new(&format!("Unexpected stat log status {status}")),
    };
    let category = match ctx.access.intern_str("Error", FindName::Add) {
        Ok(category) => category,
        Err(error) => return remember(ctx, error),
    };
    if let Err(error) = ctx.access.emit_log(category, error.diagnostic()) {
        return remember(ctx, error);
    }
    remember(ctx, error)
}
unsafe extern "C" fn unix_allocate(
    _opaque: *mut c_void,
    size: usize,
    out: *mut *mut c_void,
) -> i32 {
    if out.is_null() {
        return 1;
    }
    let p = unsafe { libc::malloc(size.max(1)) };
    if p.is_null() {
        1
    } else {
        unsafe { out.write(p) };
        0
    }
}
unsafe extern "C" fn unix_fatal(
    _opaque: *mut c_void,
    _failure: hp2_core::unix_text::Hp2UnixTextFailure,
) -> i32 {
    1
}

static SERVICES: crate::stat_log::StatLogServices = crate::stat_log::StatLogServices {
    actor_level,
    level_game,
    level_not_dedicated,
    game_external_batcher,
    game_local_log_file_name,
    fstr_chars,
    fstr_len,
    fstr_assign,
    fstr_append,
    stat_log_local_batcher_url: local_batcher_url,
    stat_log_local_log_dir: local_log_dir,
    stat_log_world_batcher_url: world_batcher_url,
    stat_log_world_batcher_params: world_batcher_params,
    stat_log_class_default_object: class_default,
    stat_log_static_default_object: static_default,
    stat_log_get_world_batcher_error: get_world_error,
    stat_log_set_world_batcher_error: set_world_error,
    stat_log_save_config: save_config,
    stat_log_get_world: get_world,
    stat_log_file_name: file_name,
    stat_log_file_final: file_final,
    stat_log_player_name: player_name,
    stat_log_player_secret: player_secret,
    stat_log_event_special_class: event_special_class,
    stat_log_event_special2: event_special2,
    class_iterate_packages: iterate_packages,
    package_full_name,
    dll_ext,
    file_size: crate::stat_log_io_owner::file_size,
    file_create_writer: crate::stat_log_io_owner::file_create_writer,
    file_make_directory: crate::stat_log_io_owner::file_make_directory,
    file_move: crate::stat_log_io_owner::file_move,
    archive_serialize: crate::stat_log_io_owner::archive_serialize,
    archive_flush: crate::stat_log_io_owner::archive_flush,
    archive_delete: crate::stat_log_io_owner::archive_delete,
    md5_context_new: crate::stat_log_io_owner::md5_context_new,
    md5_context_init: crate::stat_log_io_owner::md5_context_init,
    md5_context_update: crate::stat_log_io_owner::md5_context_update,
    md5_context_final: crate::stat_log_io_owner::md5_context_final,
    md5_context_delete: crate::stat_log_io_owner::md5_context_delete,
    debugf_text: debug_text,
    warn_text,
    set_requesting_exit: requesting_exit,
    proc_create,
    proc_return_code,
    launch_url,
    stat_log_actor_level_url_map: actor_level_url_map,
    stat_log_raise: raise,
};

fn policy_error(error: crate::stat_log::StatLogError) -> RuntimeError {
    match error {
        crate::stat_log::StatLogError::CheckFailed(expr) => RuntimeError::new(expr),
        crate::stat_log::StatLogError::FileMissing => {
            RuntimeError::new("Could not open StatLog file")
        }
        crate::stat_log::StatLogError::Md5Overflow => {
            RuntimeError::new("UTF-16LE text exceeds the 32-bit MD5 contract")
        }
        crate::stat_log::StatLogError::ArchiveOverflow => {
            RuntimeError::new("Stat log entry exceeds the 32-bit archive contract")
        }
    }
}
pub(crate) fn with_services<T>(
    access: &CoreAccess<'_>,
    operation: impl FnOnce(&crate::stat_log::StatLogServices) -> crate::stat_log::StatLogResult<T>,
) -> RuntimeResult<T> {
    with_context(*access, || match operation(&SERVICES) {
        Ok(value) => Ok(value),
        Err(error) => {
            if let Some(context) = context() {
                let mut first = context.error.borrow_mut();
                if first.is_none() {
                    *first = Some(policy_error(error));
                }
            }
            Err(1)
        }
    })
}

const ACTOR: ClassOps = crate::actor_owner::ACTOR_OPERATIONS;
const STAT_LOG_OPS: ClassOps = {
    let mut ops = ACTOR;
    ops.construct = construct_stat_log;
    ops.finalize_members = finalize_stat_log;
    ops
};
const STAT_LOG_FILE_OPS: ClassOps = {
    let mut ops = STAT_LOG_OPS;
    ops.construct = construct_stat_log_file;
    ops.finalize_members = finalize_stat_log_file;
    ops
};
pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    NativeClassSpec {
        short_name: "Mutator",
        package: "Engine",
        super_name: Some("Info"),
        within: "Object",
        flags: 0,
        object_flags: OBJECT_FLAGS,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<AMutator>()),
        operations: Some(ACTOR),
        properties: MUTATOR_PROPERTIES,
        defaults: &[],
    },
    NativeClassSpec {
        short_name: "StatLog",
        package: "Engine",
        super_name: Some("Info"),
        within: "Object",
        flags: 4,
        object_flags: OBJECT_FLAGS,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<AStatLog>()),
        operations: Some(STAT_LOG_OPS),
        properties: STAT_LOG_PROPERTIES,
        defaults: &[],
    },
    NativeClassSpec {
        short_name: "StatLogFile",
        package: "Engine",
        super_name: Some("StatLog"),
        within: "Object",
        flags: 4,
        object_flags: OBJECT_FLAGS,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<AStatLogFile>()),
        operations: Some(STAT_LOG_FILE_OPS),
        properties: STAT_LOG_FILE_PROPERTIES,
        defaults: &[],
    },
];
unsafe fn init_string(array: *mut TArrayRaw) {
    unsafe {
        array.write_unaligned(TArrayRaw {
            data: ptr::null_mut(),
            num: 0,
            max: 0,
        })
    }
}
unsafe fn destroy_string(access: CoreAccess<'_>, array: *mut TArrayRaw) -> RuntimeResult<()> {
    let a = unsafe { array.read_unaligned() };
    if !a.data.is_null() {
        unsafe { access.destroy_owned_fstring(array) }?
    }
    Ok(())
}
unsafe fn construct_stat_log(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (ACTOR.construct)(access, object)? };
    let log = object.cast::<AStatLog>();
    let fields = unsafe {
        [
            ptr::addr_of_mut!((*log).local_standard),
            ptr::addr_of_mut!((*log).world_standard),
            ptr::addr_of_mut!((*log).log_version),
            ptr::addr_of_mut!((*log).log_info_url),
            ptr::addr_of_mut!((*log).game_name),
            ptr::addr_of_mut!((*log).game_creator),
            ptr::addr_of_mut!((*log).game_creator_url),
            ptr::addr_of_mut!((*log).decoder_ring_url),
            ptr::addr_of_mut!((*log).local_batcher_url),
            ptr::addr_of_mut!((*log).local_batcher_params),
            ptr::addr_of_mut!((*log).local_stats_url),
            ptr::addr_of_mut!((*log).world_batcher_url),
            ptr::addr_of_mut!((*log).world_batcher_params),
            ptr::addr_of_mut!((*log).world_stats_url),
            ptr::addr_of_mut!((*log).local_log_dir),
            ptr::addr_of_mut!((*log).world_log_dir),
        ]
    };
    for field in fields {
        unsafe { init_string(field) }
    }
    Ok(())
}
unsafe fn construct_stat_log_file(
    access: CoreAccess<'_>,
    object: *mut UObject,
) -> RuntimeResult<()> {
    unsafe { construct_stat_log(access, object)? };
    let file = object.cast::<AStatLogFile>();
    unsafe {
        init_string(ptr::addr_of_mut!((*file).stat_log_file));
        init_string(ptr::addr_of_mut!((*file).stat_log_final))
    };
    Ok(())
}
unsafe fn finalize_stat_log(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() {
        return Err(RuntimeError::new("Null canonical AStatLog"));
    }
    let runtime = with_services(&access, |svc| {
        unsafe { crate::stat_log::destroy_runtime_state(svc, object.cast()) };
        Ok(())
    });
    let log = object.cast::<AStatLog>();
    let fields = unsafe {
        [
            ptr::addr_of_mut!((*log).local_standard),
            ptr::addr_of_mut!((*log).world_standard),
            ptr::addr_of_mut!((*log).log_version),
            ptr::addr_of_mut!((*log).log_info_url),
            ptr::addr_of_mut!((*log).game_name),
            ptr::addr_of_mut!((*log).game_creator),
            ptr::addr_of_mut!((*log).game_creator_url),
            ptr::addr_of_mut!((*log).decoder_ring_url),
            ptr::addr_of_mut!((*log).local_batcher_url),
            ptr::addr_of_mut!((*log).local_batcher_params),
            ptr::addr_of_mut!((*log).local_stats_url),
            ptr::addr_of_mut!((*log).world_batcher_url),
            ptr::addr_of_mut!((*log).world_batcher_params),
            ptr::addr_of_mut!((*log).world_stats_url),
            ptr::addr_of_mut!((*log).local_log_dir),
            ptr::addr_of_mut!((*log).world_log_dir),
        ]
    };
    let mut fields_result = Ok(());
    for field in fields {
        if let Err(error) = unsafe { destroy_string(access, field) } {
            if fields_result.is_ok() {
                fields_result = Err(error);
            }
        }
    }
    runtime.and(fields_result)
}

unsafe fn finalize_stat_log_file(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() {
        return Err(RuntimeError::new("Null canonical AStatLogFile"));
    }
    let file = object.cast::<AStatLogFile>();
    let first = unsafe { destroy_string(access, ptr::addr_of_mut!((*file).stat_log_file)) };
    let second = unsafe { destroy_string(access, ptr::addr_of_mut!((*file).stat_log_final)) };
    first.and(second).and(unsafe { finalize_stat_log(access, object) })
}
