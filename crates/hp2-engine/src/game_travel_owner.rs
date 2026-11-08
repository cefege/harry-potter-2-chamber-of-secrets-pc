//! Canonical Core object and lifetime operations for the shared UnGame travel policy.

use crate::{game_travel::{self, Op, TravelOps}, url_owner::{UrlDefaults, UrlOwner}};
use hp2_abi::engine::generated::{FURL, TArrayRaw, TMapStringString, UGameEngine, ULevel, UObject, UViewport, UClient, UPendingLevel};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use std::{cell::RefCell, mem::MaybeUninit, path::PathBuf, ptr};

const EMPTY: &[u32] = &[0];
const LINK: &[u32] = &[46,117,110,114,101,97,108,0];
const OPTIONS: [&[u32]; 4] = [
    &[102,97,105,108,101,100,0], &[101,110,116,114,121,0],
    &[112,111,112,0], &[114,101,115,116,97,114,116,0],
];
const LOAD: &[u32] = &[108,111,97,100,0];
const LOAD_PREFIX: &[u32] = &[108,111,97,100,61,0];

#[derive(Clone, Copy)]
struct Context<'a> {
    access: CoreAccess<'a>,
    engine: *mut UGameEngine,
    url: *mut FURL,
    travel_info: *const TMapStringString,
    error: *mut TArrayRaw,
    match_client: *mut UClient,
    match_level: *mut ULevel,
    scratch: *mut TArrayRaw,
    temp: *mut TArrayRaw,
    save_url: *mut FURL,
    filename: *mut u32,
    file_token: *mut u32,
    portal: *mut u32,
    source: *mut u32,
    dest: *mut u32,
    slot: i32,
}

impl<'a> Context<'a> {
    fn new(access: CoreAccess<'a>, engine: *mut UGameEngine, url: *mut FURL) -> Self {
        Self {
            access, engine, url, travel_info: ptr::null(), error: ptr::null_mut(),
            match_client: ptr::null_mut(), match_level: ptr::null_mut(),
            scratch: ptr::null_mut(), temp: ptr::null_mut(), save_url: ptr::null_mut(),
            filename: ptr::null_mut(), file_token: ptr::null_mut(), portal: ptr::null_mut(),
            source: ptr::null_mut(), dest: ptr::null_mut(), slot: 0,
        }
    }
}

struct CoreTravelOps<'a, 'd> {
    context: Context<'a>,
    defaults: &'d UrlDefaults,
    text: RefCell<Vec<Vec<u32>>>,
}

fn empty_string() -> TArrayRaw { TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 } }

unsafe fn units<'a>(value: *const TArrayRaw) -> &'a [u32] {
    let value = unsafe { &*value };
    if value.num <= 1 { &[] } else { unsafe { std::slice::from_raw_parts(value.data.cast(), value.num as usize - 1) } }
}

unsafe fn data(value: *const TArrayRaw) -> *const u32 {
    if unsafe { (*value).data.is_null() } { EMPTY.as_ptr() } else { unsafe { (*value).data.cast() } }
}

unsafe fn text_units<'a>(text: *const u32) -> &'a [u32] {
    let mut len = 0;
    while unsafe { *text.add(len) } != 0 { len += 1; }
    unsafe { std::slice::from_raw_parts(text, len) }
}

fn finish<T, const N: usize>(mut result: RuntimeResult<T>, cleanups: [RuntimeResult<()>; N]) -> RuntimeResult<T> {
    for cleanup in cleanups {
        if let Err(error) = cleanup { if result.is_ok() { result = Err(error); } }
    }
    result
}

impl<'a, 'd> CoreTravelOps<'a, 'd> {
    fn new(context: Context<'a>, defaults: &'d UrlDefaults) -> Self {
        Self { context, defaults, text: RefCell::new(Vec::new()) }
    }

    fn resume(&self, context: Context<'a>, entry: u32, arg: usize) -> RuntimeResult<usize> {
        game_travel::run_with_ops(&Self::new(context, self.defaults), entry, arg)
    }

    fn keep_text(&self, mut text: Vec<u32>) -> usize {
        if text.last() != Some(&0) { text.push(0); }
        let address = text.as_ptr() as usize;
        self.text.borrow_mut().push(text);
        address
    }

    fn log(&self, message: &str) -> RuntimeResult<()> {
        let category = self.context.access.intern_str("Log", FindName::Add)?;
        self.context.access.emit_log(category, &message.chars().map(u32::from).collect::<Vec<_>>())
    }

    fn log_text(&self, prefix: &str, text: &[u32]) -> RuntimeResult<()> {
        let category = self.context.access.intern_str("Log", FindName::Add)?;
        let mut message = Vec::with_capacity(prefix.len() + text.len());
        message.extend(prefix.bytes().map(u32::from));
        message.extend_from_slice(text);
        self.context.access.emit_log(category, &message)
    }

    fn object_name(&self, object: *mut UObject) -> RuntimeResult<Vec<u32>> {
        if object.is_null() { return Err(RuntimeError::new("Null object in canonical game travel")); }
        self.context.access.name_text(i32::from_ne_bytes(unsafe { (*object).name }))
    }

    fn localize(&self, key: &str) -> RuntimeResult<Vec<u32>> {
        let mut units = [0; 32];
        for (index, byte) in key.bytes().enumerate() { units[index] = u32::from(byte); }
        self.context.access.localize_text(&[69,114,114,111,114,115], &units[..key.len()], Some(&[69,110,103,105,110,101]), None, false)
    }

    fn set_error(&self, text: &[u32]) -> RuntimeResult<usize> {
        unsafe { self.context.access.assign_fstring(self.context.error, text)?; }
        Ok(0)
    }

    fn formatted_error(&self, key: &str, argument: &[u32]) -> RuntimeResult<usize> {
        use hp2_core::printf_format::{self, DirectiveAction, Length};
        let mut format = self.localize(key)?;
        format.push(0);
        let mut output = Vec::with_capacity(format.len() + argument.len());
        let mut index = 0;
        let mut consumed = false;
        while format[index] != 0 {
            let ch = format[index]; index += 1;
            if ch != u32::from('%') { output.push(ch); continue; }
            if format[index] == u32::from('%') { output.push(u32::from('%')); index += 1; continue; }
            let mut segment = [0; 64]; segment[0] = '%' as hp2_core::ansi::Tchar;
            let mut written = 1;
            let mut length = Length::None;
            let mut conversion = 0;
            let action = unsafe { printf_format::parse_directive(
                format.as_ptr().cast(), &mut index, segment.as_mut_ptr(), segment.len(),
                &mut written, 0, 0, &mut length, &mut conversion,
            ) };
            // These native call sites supply precisely one wide-text argument.
            // appVswprintfCompat ignores width/precision for wide strings.
            if action != DirectiveAction::Done || conversion != 's' as hp2_core::ansi::Tchar
                || matches!(length, Length::H | Length::Hh) || consumed {
                return Err(RuntimeError::new("Invalid localized single-text travel format").with_context(key));
            }
            output.extend_from_slice(argument); consumed = true;
        }
        output.push(0);
        self.set_error(&output)
    }

    fn slot_file(&self, filename: &str) -> RuntimeResult<PathBuf> {
        Ok(crate::game_init_owner::system_path(self.context.access, true)?.join(filename))
    }

    unsafe fn write_path(&self, output: *mut u32, filename: &str) -> RuntimeResult<()> {
        let units = hp2_core::native_path::path_to_units(&self.slot_file(filename)?)?;
        if units.len() >= 256 { return Err(RuntimeError::new("Travel filename exceeds native buffer")); }
        unsafe { ptr::copy_nonoverlapping(units.as_ptr(), output, units.len()); *output.add(units.len()) = 0; }
        Ok(())
    }

    unsafe fn path(&self, text: *const u32) -> RuntimeResult<PathBuf> {
        hp2_core::native_path::path_from_units(unsafe { text_units(text) })
    }

    fn set_enabled(&self, object: *mut UObject) -> RuntimeResult<()> {
        let access = self.context.access;
        let name = access.intern_str("bEnabled", FindName::Find)?;
        let bool_class = access.native_class("BoolProperty")?;
        let property = unsafe { access.find_field((*object).class.cast(), name, bool_class) };
        if property.is_null() { return Err(RuntimeError::new("PlayerStart bEnabled property is missing")); }
        let property = property.cast::<hp2_abi::engine::generated::UBoolProperty>();
        let mask = unsafe { (*property).bit_mask };
        let address = unsafe { access.property_address(object.cast(), property.cast())? }.cast::<u32>();
        unsafe { address.write_unaligned(address.read_unaligned() & !mask); }
        Ok(())
    }
}

impl TravelOps for CoreTravelOps<'_, '_> {
    type Error = RuntimeError;

    fn call(&self, op: Op, a: usize, b: usize) -> RuntimeResult<usize> {
        let c = self.context;
        let access = c.access;
        unsafe {
            Ok(match op {
                Op::ClearError => return self.set_error(EMPTY),
                Op::MapText => data(ptr::addr_of!((*c.url).map)) as usize,
                Op::FindLink => hp2_core::ansi::strstr(data(ptr::addr_of!((*c.url).map)).cast(), LINK.as_ptr().cast()) as usize,
                Op::TextLen => text_units(a as *const u32).len(),
                Op::LinkText => LINK.as_ptr() as usize,
                Op::LogLink => { self.log_text("Link: ", units(ptr::addr_of!((*c.url).map)))?; 0 }
                Op::ReadLink => {
                    let found = access.config_get_units_from(Some(units(ptr::addr_of!((*c.url).map))), &[76,105,110,107], &[83,101,114,118,101,114])?;
                    if let Some(mut text) = found { text.push(0); access.assign_fstring(c.scratch, &text)?; 1 } else { 0 }
                }
                Op::AssignLink => { UrlOwner::parse_update(access, self.defaults, c.url, ptr::null_mut(), data(c.scratch), 0)?; 0 }
                Op::InvalidLinkError => return self.formatted_error("InvalidLink", units(ptr::addr_of!((*c.url).map))),
                Op::LogBrowse => {
                    let mut text = UrlOwner::string(access, self.defaults, c.url, false)?;
                    let result = self.log_text("Browse: ", units(&text));
                    finish(result, [access.destroy_owned_fstring(&mut text)])?; 0
                }
                Op::UrlValid => (*c.url).valid as usize,
                Op::InvalidUrlError => {
                    let mut text = UrlOwner::string(access, self.defaults, c.url, false)?;
                    let result = self.formatted_error("InvalidUrl", units(&text));
                    return finish(result, [access.destroy_owned_fstring(&mut text)]);
                }
                Op::HasOption => usize::from(UrlOwner::has_option(access, self.defaults, c.url, OPTIONS[a].as_ptr())?),
                Op::LoadOption => UrlOwner::get_option(access, self.defaults, c.url, LOAD_PREFIX.as_ptr(), ptr::null())? as usize,
                Op::LogAbort => { let category = access.intern_str("Log", FindName::Add)?; access.emit_log(category, &self.localize("AbortToEntry")?)?; 0 }
                Op::Level => (*c.engine).g_level as usize,
                Op::Entry => (*c.engine).g_entry as usize,
                Op::Brush => (*(a as *mut ULevel)).brush_tracker as usize,
                Op::DeleteBrush => { crate::dynbsp_owner::destroy(access, a as *mut ULevel)?; 0 }
                Op::ClearBrush => { (*(a as *mut ULevel)).brush_tracker = ptr::null_mut(); 0 }
                Op::ResetLoaders => { access.reset_loaders((*(a as *mut UObject)).outer.cast(), true, false)?; 0 }
                Op::Notify => { game_travel::run_with_ops(self, 102, 0)?; 0 }
                Op::SetLevel => { (*c.engine).g_level = a as *mut _; 0 }
                Op::ClearAction => { let info = crate::package_map_level_owner::level_info(a as *mut ULevel)?; (*info).level_action = 0; 0 }
                Op::CheckClient => return Err(RuntimeError::new("Client && Client->Viewports.Num()")),
                Op::Client => (*c.engine).client as usize,
                Op::MatchClient => c.match_client as usize,
                Op::ViewportCount => (*(a as *mut UClient)).viewports.num as usize,
                Op::Viewport => (*(a as *mut UClient)).viewports.data.cast::<*mut UViewport>().add(b).read_unaligned() as usize,
                Op::Console => (*(a as *mut UViewport)).console as usize,
                Op::ConsoleNotify => { access.execute_named_event(a as *mut UObject, "NotifyLevelChange")?; 0 }
                Op::Audio => (*c.engine).audio as usize,
                Op::AudioViewport => (*(a as *mut hp2_abi::engine::generated::UALAudioSubsystem)).viewport as usize,
                Op::AudioSetViewport => { crate::audio_owner::set_viewport(access, a as *mut _, b as *mut UViewport)?; 0 }
                Op::Pending => (*c.engine).g_pending_level as usize,
                Op::ConnectionProgress => { let message = self.keep_text(self.localize("ConnectionFailed")?); crate::game_engine_owner::set_progress(access, c.engine, message as *const u32, EMPTY.as_ptr(), 6.0)?; 0 }
                Op::HubDepth => { let info = crate::package_map_level_owner::level_info(a as *mut ULevel)?; (*info).hub_stack_level as usize }
                Op::PopFilename => { self.write_path(c.filename, &format!("Game{}.usa", a as i32))?; 0 }
                Op::PopToken => usize::from(crate::url::file_path_to_token(c.filename.cast(), c.file_token.cast(), 256)),
                Op::SavePortal => {
                    let portal = units(ptr::addr_of!((*c.url).portal));
                    if portal.len() >= 256 { return Err(RuntimeError::new("Travel portal exceeds native buffer")); }
                    ptr::copy_nonoverlapping(portal.as_ptr(), c.portal, portal.len()); *c.portal.add(portal.len()) = 0; 0
                }
                Op::PopUrl => { UrlOwner::parse_update(access, self.defaults, c.url, c.url, c.file_token, 1)?; 0 }
                Op::RestorePortal => { access.assign_fstring(ptr::addr_of_mut!((*c.url).portal), std::slice::from_raw_parts(c.portal, text_units(c.portal).len() + 1))?; 0 }
                Op::Restart => { UrlOwner::assign_copy(access, c.url, ptr::addr_of!((*c.engine).last_url))?; 0 }
                Op::Atoi => hp2_core::ansi::atoi_checked((a as *const u32).cast())
                    .map_err(|_| RuntimeError::new("Travel slot integer is outside the native INT range"))? as usize,
                Op::AddLoad => { UrlOwner::add_option(access, self.defaults, c.save_url, LOAD.as_ptr())?; 0 }
                Op::LoadSaved => crate::game_load_map_owner::load_map(access, c.engine, c.save_url, ptr::null_mut(), ptr::null(), c.error)? as usize,
                Op::CopySource => { self.write_path(c.source, &format!("Save{}{}.usa", c.slot, a as i32))?; 0 }
                Op::CopyDest => { self.write_path(c.dest, &format!("Game{}.usa", a as i32))?; 0 }
                // Preserve the native (Source, Dest) arguments to the (Dest, Source) API.
                Op::CopyFile => { hp2_core::unix_file_manager_owner::copy_file_paths(access, &self.path(c.source)?, &self.path(c.dest)?, false)?; 0 }
                Op::GameFilename => {
                    let path = self.slot_file(&format!("Game{}.usa", a as i32))?;
                    let mut text = hp2_core::native_path::path_to_units(&path)?; text.push(0);
                    access.assign_fstring(c.temp, &text)?; 0
                }
                Op::FileSize => hp2_core::unix_file_manager_owner::file_size(access, &self.path(data(c.temp))?)? as usize,
                Op::DeleteFile => { hp2_core::unix_file_manager_owner::delete_file_path(access, &self.path(data(c.temp))?, false)?; 0 }
                Op::RepairSlot => crate::game_tick_owner::repair_save_slot(access)? as usize,
                Op::RepairPending => crate::game_tick_owner::repair_save_pending(access)? as usize,
                Op::SetRepairPending => { crate::game_tick_owner::set_repair_save_pending(access, 1)?; 0 }
                Op::LastLevelUrl => { UrlOwner::assign_copy(access, ptr::addr_of_mut!((*c.engine).last_url), ptr::addr_of!((*(*c.engine).g_level.cast::<ULevel>()).base.url))?; 0 }
                Op::IsLocal => usize::from(UrlOwner::is_local_internal(access, self.defaults, c.url)?),
                Op::LoadLocal => crate::game_load_map_owner::load_map(access, c.engine, c.url, ptr::null_mut(), c.travel_info, c.error)? as usize,
                Op::IsInternal => usize::from(UrlOwner::is_internal(access, self.defaults, c.url)?),
                Op::IsClient => usize::from(access.process_mode().is_client),
                Op::CancelPending => { crate::network_owner::cancel_game_pending(access, c.engine)?; 0 }
                Op::NewPending => crate::network_owner::new_pending(access, c.engine, c.url)? as usize,
                Op::SetPending => { (*c.engine).g_pending_level = a as *mut _; 0 }
                Op::PendingDriver => (*(a as *mut UPendingLevel)).net_driver as usize,
                Op::NetworkProgress => {
                    let message: &[u32] = &[78,101,116,119,111,114,107,105,110,103,32,70,97,105,108,101,100,0];
                    let pending = (*c.engine).g_pending_level.cast::<UPendingLevel>();
                    crate::game_engine_owner::set_progress(access, c.engine, message.as_ptr(), data(ptr::addr_of!((*pending).error)), 6.0)?; 0
                }
                Op::DeletePending => { access.delete_object(a as *mut UObject)?; 0 }
                Op::ServerError => { let mut text = self.localize("ServerOpen")?; text.push(0); return self.set_error(&text); }
                // The authoritative Unix appLaunchURL body is empty.
                Op::Launch => { hp2_core::unix_process::launch_url(); 0 }
                Op::LogSpawn => { self.log_text("Spawning new actor for Viewport ", &self.object_name(a as *mut UObject)?)?; 0 }
                Op::Spawn => crate::level_player_owner::spawn_play_actor(access, c.match_level, (a as *mut UViewport).cast(), 2, c.url, c.scratch)? as usize,
                Op::SpawnError => return Err(RuntimeError::from_utf32(units(c.scratch).to_vec())),
                Op::FullName => self.keep_text(access.object_full_name(a as *mut UObject)?),
                Op::ObjectName => self.keep_text(self.object_name(a as *mut UObject)?),
                Op::CompareText => {
                    const NAMES: [&[u32]; 4] = [
                        &[76,101,118,101,108,32,67,84,70,45,67,111,114,101,116,46,77,121,76,101,118,101,108,0],
                        &[65,108,116,101,114,110,97,116,101,80,97,116,104,49,50,0],
                        &[65,108,116,101,114,110,97,116,101,80,97,116,104,49,51,0],
                        &[80,108,97,121,101,114,83,116,97,114,116,53,0],
                    ];
                    hp2_core::ansi::stricmp((a as *const u32).cast(), NAMES[b].as_ptr().cast()) as usize
                }
                Op::LogFix => { self.log("Fixing up CTF-Coret")?; 0 }
                Op::Navigation => (*crate::package_map_level_owner::level_info(a as *mut ULevel)?).navigation_point_list as usize,
                Op::LogFixed => { self.log_text("Fixed up ", &self.object_name(a as *mut UObject)?)?; 0 }
                Op::TwoWay => { (*(a as *mut hp2_abi::engine::generated::ANavigationPoint)).set_b_two_way(true); 0 }
                Op::PlayerStart => {
                    let class = access.native_class("Engine.PlayerStart")?;
                    if hp2_core::object_model::is_a(a as *const _, class.cast()) { a } else { 0 }
                }
                Op::DisableStart => { self.set_enabled(a as *mut UObject)?; 0 }
                Op::NextNavigation => (*(a as *mut hp2_abi::engine::generated::ANavigationPoint)).next_navigation_point as usize,
                Op::LogLevel => { self.log_text("Level is ", &access.object_full_name((*c.engine).g_level.cast())?)?; 0 }
                Op::CheckEnabled => usize::from(hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.checks != 0),
            })
        }
    }

    fn scope(&self, entry: u32, arg: usize) -> RuntimeResult<usize> {
        let c = self.context;
        let access = c.access;
        unsafe {
            match entry {
                4 => {
                    let mut filename = MaybeUninit::<[u32; 256]>::uninit();
                    let mut token = MaybeUninit::<[u32; 256]>::uninit();
                    let mut portal = MaybeUninit::<[u32; 256]>::uninit();
                    self.resume(Context { filename: filename.as_mut_ptr().cast(), file_token: token.as_mut_ptr().cast(), portal: portal.as_mut_ptr().cast(), ..c }, entry, arg)
                }
                11 | 13 => {
                    let mut scratch = empty_string();
                    let result = self.resume(Context { scratch: &mut scratch, ..c }, entry, arg);
                    finish(result, [access.destroy_owned_fstring(&mut scratch)])
                }
                12 => {
                    let slot = arg as i32;
                    let mut error = empty_string();
                    let mut temp = empty_string();
                    let mut path = hp2_core::native_path::path_to_units(&self.slot_file(&format!("Save{slot}.usa"))?)?;
                    path.push(0);
                    access.assign_fstring(&mut temp, &path)?;
                    let mut save_url = MaybeUninit::<FURL>::uninit();
                    if let Err(error) = UrlOwner::construct_local_filename(access, self.defaults, save_url.as_mut_ptr(), Some(std::slice::from_raw_parts(data(&temp), temp.num as usize))) {
                        let _ = access.destroy_owned_fstring(&mut temp);
                        return Err(error);
                    }
                    let mut source = MaybeUninit::<[u32; 256]>::uninit();
                    let mut dest = MaybeUninit::<[u32; 256]>::uninit();
                    let result = self.resume(Context { slot, error: &mut error, temp: &mut temp, save_url: save_url.as_mut_ptr(), source: source.as_mut_ptr().cast(), dest: dest.as_mut_ptr().cast(), ..c }, entry, arg);
                    finish(result, [UrlOwner::destroy(access, save_url.as_mut_ptr()), access.destroy_owned_fstring(&mut temp), access.destroy_owned_fstring(&mut error)])
                }
                14 => self.resume(Context { match_client: (*c.engine).client.cast(), match_level: (*c.engine).g_level.cast(), ..c }, 101, 0),
                1..=10 => game_travel::run_with_ops(self, entry, arg),
                _ => Err(RuntimeError::new("Invalid canonical game travel scope").with_context(&entry.to_string())),
            }
        }
    }
}

/// Executes the source by-value Browse policy without mutating the caller's URL.
///
/// # Safety
/// Engine, URL and error are live canonical records; optional travel data is live.
pub unsafe fn browse(access: CoreAccess<'_>, engine: *mut UGameEngine, url: *const FURL, travel_info: *const TMapStringString, error: *mut TArrayRaw) -> RuntimeResult<bool> {
    if engine.is_null() || url.is_null() || error.is_null() { return Err(RuntimeError::new("Invalid canonical GameEngine Browse arguments")); }
    let defaults = UrlOwner::current_defaults(access)?;
    let mut copied = MaybeUninit::<FURL>::uninit();
    unsafe { UrlOwner::copy_construct(access, copied.as_mut_ptr(), url)?; }
    let context = Context { travel_info, error, ..Context::new(access, engine, copied.as_mut_ptr()) };
    let result = game_travel::run_with_ops(&CoreTravelOps::new(context, &defaults), 100, 0).map(|value| value != 0);
    finish(result, [unsafe { UrlOwner::destroy(access, copied.as_mut_ptr()) }])
}

/// Executes matching, notification and legacy level fixup with the same provider.
///
/// # Safety
/// All supplied canonical records remain live during the shared policy call.
pub(crate) unsafe fn run(access: CoreAccess<'_>, engine: *mut UGameEngine, level: *mut ULevel, url: *const FURL, entry: u32, arg: usize) -> RuntimeResult<usize> {
    let defaults = UrlOwner::current_defaults(access)?;
    let context = Context { match_client: unsafe { (*engine).client.cast() }, match_level: level, ..Context::new(access, engine, url.cast_mut()) };
    game_travel::run_with_ops(&CoreTravelOps::new(context, &defaults), entry, arg)
}
