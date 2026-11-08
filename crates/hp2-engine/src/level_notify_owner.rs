//! Canonical Core adapter for `ULevel::NotifyReceivedText`.

use hp2_abi::engine::generated::{
    AGameInfo, AGameInfoPreLoginParams, APawn, FPackageInfo, FURL, TArrayRaw, UClient,
    UControlChannel, UGameEngine, ULevel, UNetConnection, UNetDriver, UFunction, UPackageMap,
    UViewport,
};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::name_owner::FindName;
use std::ptr;

/// Executes the shared text policy against canonical Core-owned records.
///
/// # Safety
/// `level` and `connection` must be live records owned by `access`; `text` must
/// be readable as a NUL-terminated TCHAR string for this call.
pub unsafe fn notify_received_text(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    connection: *mut UNetConnection,
    text: *const u32,
) -> RuntimeResult<()> {
    if level.is_null() || connection.is_null() || text.is_null() {
        return Err(RuntimeError::new("Invalid Level text notification arguments"));
    }
    let mut ops = CoreNotifyReceivedTextOps {
        access,
        join_error: ptr::null_mut(),
        prelogin_params: ptr::null_mut(),
    };
    // SAFETY: this wrapper preserves the caller's live-record and text-pointer contract.
    unsafe {
        crate::level_net::notify_received_text_with(
            &mut ops,
            level.cast(),
            connection.cast(),
            text,
        )
    }
}

struct CoreNotifyReceivedTextOps<'a> {
    access: CoreAccess<'a>,
    join_error: *mut TArrayRaw,
    prelogin_params: *mut AGameInfoPreLoginParams,
}

impl CoreNotifyReceivedTextOps<'_> {
    fn invalid(&self, message: &'static str) -> RuntimeError { RuntimeError::new(message) }

    fn log(&self, category: &str, text: *const u32) -> RuntimeResult<()> {
        if text.is_null() { return Err(self.invalid("Null Core log text")); }
        let mut len = 0usize;
        while unsafe { *text.add(len) } != 0 { len += 1; }
        let name = self.access.intern_str(category, FindName::Add)?;
        // SAFETY: policy messages are live NUL-terminated TCHAR buffers.
        self.access.emit_log(name, unsafe { std::slice::from_raw_parts(text, len) })
    }

    unsafe fn package_list(&self, map: *mut UPackageMap) -> RuntimeResult<TArrayRaw> {
        if map.is_null() { return Err(self.invalid("Connection has no PackageMap")); }
        let list = unsafe { ptr::addr_of!((*map).list).read_unaligned() };
        if list.num < 0 || list.max < list.num || (list.num > 0 && list.data.is_null()) {
            return Err(self.invalid("Invalid PackageMap List storage"));
        }
        Ok(list)
    }


    unsafe fn prelogin_event(&self, game: *mut AGameInfo) -> RuntimeResult<*mut UFunction> {
        if game.is_null() { return Err(self.invalid("LevelInfo has no Game")); }
        let name = self.access.intern_str("PreLogin", FindName::Find)?;
        let event = unsafe { self.access.find_object_function_by_name(game.cast(), name, false)? };
        if event.is_null() { return Err(self.invalid("AGameInfo has no PreLogin event")); }
        if unsafe { (*event).parms_size } as usize != std::mem::size_of::<AGameInfoPreLoginParams>()
            || unsafe { (*event).num_parms } != 4
        {
            return Err(self.invalid("PreLogin event parameter layout mismatch"));
        }
        Ok(event)
    }
}

impl crate::level_net::NotifyReceivedTextOps for CoreNotifyReceivedTextOps<'_> {
    type Error = RuntimeError;

    fn invalid_error(&mut self) -> Self::Error { RuntimeError::new("Engine->Client->Viewports.Num()") }

    unsafe fn call(
        &mut self,
        op: crate::level_net::NotifyTextOp,
        a: usize,
        b: usize,
        c: usize,
    ) -> RuntimeResult<usize> {
        use crate::level_net::NotifyTextOp as Op;
        let p = |value: usize| value as *mut std::ffi::c_void;
        let result = match op {
            Op::ParseCommand => {
                let stream = a as *mut *const u32;
                usize::from(unsafe { hp2_core::command::parse_command(stream.cast(), (b as *const u32).cast()) != 0 })
            }
            Op::ParseInt => usize::from(parse_int(a as *const u32, b as *const u32, c as *mut i32)),
            Op::ParseGuid => {
                let guid = parse_guid(a as *const u32, b as *const u32);
                unsafe { ptr::copy_nonoverlapping(guid.as_ptr(), c as *mut u32, 4); }
                1
            }
            Op::ParseText => {
                let output = c as *mut u32;
                unsafe {
                    hp2_core::text_parse::parse_string(
                        (a as *const u32).cast(),
                        (b as *const u32).cast(),
                        output.cast(),
                        1024,
                    );
                }
                1
            },
            Op::AppAtoi => atoi(a as *const u32) as usize,
            Op::AppCycles => hp2_core::unix_clock::cycles() as usize,
            Op::LevelEngine => unsafe { (*p(a).cast::<ULevel>()).base.engine as usize },
            Op::LevelNetDriver => unsafe { (*p(a).cast::<ULevel>()).base.net_driver as usize },
            Op::NetDriverServerConnection => unsafe { (*p(a).cast::<UNetDriver>()).server_connection as usize },
            Op::CheckFailed => {
                let expression = a as *const i8;
                if expression.is_null() { return Err(self.invalid("Null check expression")); }
                let expression = unsafe { std::ffi::CStr::from_ptr(expression) }.to_string_lossy();
                let diagnostic = format!("Assertion failed: {expression}, file UnLevel.cpp, line {b}");
                let message = diagnostic.chars().map(u32::from).collect::<Vec<_>>();
                let category = self.access.intern_str("Error", FindName::Add)?;
                self.access.emit_log(category, &message)?;
                1
            }
            Op::DebugDevNet => { self.log("DevNet", b as *const u32)?; 0 }
            Op::DebugLog => { self.log("Log", b as *const u32)?; 0 }
            Op::EngineClient => {
                let engine = p(a).cast::<UGameEngine>();
                if engine.is_null() { return Err(self.invalid("Level has no GameEngine")); }
                unsafe { (*engine).client as usize }
            }
            Op::EngineClientViewportsNum => {
                let client = p(a).cast::<UClient>();
                if client.is_null() { return Err(self.invalid("Null Engine Client")); }
                unsafe { ptr::addr_of!((*client).viewports).read_unaligned().num.max(0) as usize }
            }
            Op::EngineClientViewport => {
                let client = p(a).cast::<UClient>();
                if client.is_null() { return Err(self.invalid("Null Engine Client")); }
                let viewports = unsafe { ptr::addr_of!((*client).viewports).read_unaligned() };
                if viewports.num <= 0 || viewports.data.is_null() || viewports.max < viewports.num {
                    return Err(self.invalid("Invalid Engine Client Viewports storage"));
                }
                let viewport = unsafe { viewports.data.cast::<*mut UViewport>().read() };
                if viewport.is_null() { return Err(self.invalid("Null first Engine Client viewport")); }
                let class = self.access.native_class("Engine.Viewport")?;
                if unsafe { !hp2_core::object_model::is_a(viewport.cast(), class.cast()) } {
                    return Err(self.invalid("Engine::SetClientTravel player is not a Viewport"));
                }
                viewport as usize
            }
            Op::EngineSetClientTravel => {
                let viewport = p(b).cast::<UViewport>();
                if viewport.is_null() { return Err(self.invalid("Null first Engine Client viewport")); }
                unsafe {
                    crate::game_engine_owner::set_client_travel(self.access, viewport.cast(), c as *const u32,
                        0, crate::level_net::TRAVEL_ABSOLUTE)?;
                }
                0
            }
            Op::ConnectionSetUserFlags => { unsafe { (*p(a).cast::<UNetConnection>()).user_flags = b as i32; } 0 }
            Op::ConnectionSetRemoteVersion => { unsafe { (*p(a).cast::<UNetConnection>()).remote_version = b as i32; } 0 }
            Op::ConnectionSetNegotiatedVersion => { unsafe { (*p(a).cast::<UNetConnection>()).negotiated_ver = b as i32; } 0 }
            Op::LevelGetInfo => unsafe { crate::package_map_level_owner::level_info(p(a).cast())? as usize },
            Op::LevelInfoGame => {
                let info = p(a).cast::<hp2_abi::engine::generated::ALevelInfo>();
                if info.is_null() { return Err(self.invalid("Level has no LevelInfo")); }
                unsafe { (*info).game as usize }
            }
            Op::GameWorldLog => {
                let game = p(a).cast::<AGameInfo>();
                if game.is_null() { return Err(self.invalid("LevelInfo has no Game")); }
                usize::from(unsafe { (*game).b_world_log() })
            }
            Op::ConnectionSetChallenge => { unsafe { (*p(a).cast::<UNetConnection>()).challenge = b as i32; } 0 }
            Op::ConnectionRemoteVersion => unsafe { (*p(a).cast::<UNetConnection>()).remote_version as usize },
            Op::ConnectionSetClosed => { unsafe { (*p(a).cast::<UNetConnection>()).state = crate::level_net::USOCK_CLOSED; } 0 }
            Op::ConnectionSetCurrentNetSpeed => { unsafe { (*p(a).cast::<UNetConnection>()).current_net_speed = b as i32; } 0 }
            Op::ConnectionCurrentNetSpeed => unsafe { (*p(a).cast::<UNetConnection>()).current_net_speed as usize },
            Op::NetDriverMaxClientRate => unsafe { (*p(a).cast::<UNetDriver>()).max_client_rate as usize },
            Op::ConnectionPackageMap => unsafe { (*p(a).cast::<UNetConnection>()).package_map as usize },
            Op::PackageMapListNum => {
                let map = p(a).cast::<UPackageMap>();
                if map.is_null() { return Err(self.invalid("Connection has no PackageMap")); }
                unsafe { self.package_list(map)?.num as usize }
            }
            Op::PackageInfoGuidEqual => {
                let map = p(a).cast::<UPackageMap>(); let list = unsafe { self.package_list(map)? };
                if b >= list.num as usize { return Err(self.invalid("PackageMap index out of range")); }
                let info = unsafe { &*list.data.cast::<FPackageInfo>().add(b) };
                let guid = unsafe { std::slice::from_raw_parts(c as *const u32, 4) };
                usize::from(info.guid.as_slice() == guid)
            }
            Op::PackageInfoRemoteGeneration => {
                let list = unsafe { self.package_list(p(a).cast())? };
                if b >= list.num as usize { return Err(self.invalid("PackageMap index out of range")); }
                unsafe { ptr::addr_of_mut!((*list.data.cast::<FPackageInfo>().add(b)).remote_generation) as usize }
            }
            Op::PackageInfoUrlText => {
                let list = unsafe { self.package_list(p(a).cast())? };
                if b >= list.num as usize { return Err(self.invalid("PackageMap index out of range")); }
                let url = unsafe { (*list.data.cast::<FPackageInfo>().add(b)).url.data };
                if url.is_null() { EMPTY_TEXT.as_ptr() as usize } else { url.cast::<u32>() as usize }
            }
            Op::PackageMapRemove => {
                let map = p(a).cast::<UPackageMap>(); let list = unsafe { self.package_list(map)? };
                if b >= list.num as usize { return Err(self.invalid("PackageMap index out of range")); }
                unsafe { self.access.destroy_owned_fstring(ptr::addr_of_mut!((*list.data.cast::<FPackageInfo>().add(b)).url))?; }
                let slots = hp2_core::array_storage::Hp2ArraySlots { data: unsafe { ptr::addr_of_mut!((*map).list.data).cast() }, num: unsafe { ptr::addr_of_mut!((*map).list.num) }, max: unsafe { ptr::addr_of_mut!((*map).list.max) } };
                unsafe { hp2_core::array_storage::remove(slots, b as i32, 1, std::mem::size_of::<FPackageInfo>() as i32, ptr::null(), hp2_core::array_storage::system_allocator()).map_err(|_| self.invalid("Failed removing skipped PackageMap entry"))?; }
                0
            }
            Op::EngineChallengeResponse => crate::game_engine::challenge_response(b as i32) as usize,
            Op::ConnectionChallenge => unsafe { (*p(a).cast::<UNetConnection>()).challenge as usize },
            Op::ConnectionSetRequestUrl => {
                let connection = p(a).cast::<UNetConnection>();
                let len = nul_len(b as *const u32);
                unsafe { self.access.assign_fstring(ptr::addr_of_mut!((*connection).request_url), std::slice::from_raw_parts(b as *const u32, len + 1))?; }
                0
            }
            Op::ConnectionRequestUrlText => {
                let connection = p(a).cast::<UNetConnection>();
                let url = unsafe { (*connection).request_url.data };
                if url.is_null() { EMPTY_TEXT.as_ptr() as usize } else { url.cast::<u32>() as usize }
            }
            Op::FStringNotEmpty => {
                let value = unsafe { &*p(a).cast::<TArrayRaw>() };
                if value.num < 0 || value.max < value.num || (value.num > 0 && value.data.is_null()) {
                    return Err(self.invalid("Invalid canonical FString storage"));
                }
                usize::from(value.num > 0 && unsafe { *value.data.cast::<u32>() != 0 })
            }
            Op::FStringText => {
                let value = unsafe { &*p(a).cast::<TArrayRaw>() };
                if value.num < 0 || value.max < value.num || (value.num > 0 && value.data.is_null()) {
                    return Err(self.invalid("Invalid canonical FString storage"));
                }
                if value.num == 0 { EMPTY_TEXT.as_ptr() as usize } else { value.data.cast::<u32>() as usize }
            }
            Op::ConnectionChannelClose => {
                let conn = p(a).cast::<UNetConnection>(); let channel = unsafe { (*conn).channels[0] };
                if !channel.is_null() { unsafe { crate::channel_owner::close(self.access, channel.cast())?; } }
                0
            }
            Op::ConnectionFlushNet => { unsafe { crate::connection_owner::flush_net(self.access, p(a).cast())?; } 0 }
            Op::ConnectionLog => {
                let conn = p(a).cast::<UNetConnection>();
                let channel = unsafe { (*conn).channels[0].cast::<UControlChannel>() };
                if !channel.is_null() && unsafe { (*channel).base.closing == 0 } {
                    unsafe { crate::channel_owner::serialize_control(self.access, channel, b as *const u32)?; }
                }
                0
            }
            Op::LevelWelcomePlayer => { unsafe { crate::level_net_owner::welcome_player(self.access, p(a).cast(), p(b).cast(), ptr::null())?; } 0 }
            Op::PackageMapCompute => { unsafe { self.access.package_map_compute(p(a).cast())?; } 0 }
            Op::SpawnPlayActor => {
                let error = self.join_error;
                if error.is_null() { return Err(self.invalid("JOIN error scope is not active")); }
                let actor = unsafe { crate::level_player_owner::spawn_play_actor(self.access, p(a).cast(), p(b).cast(), crate::level_net::ROLE_AUTONOMOUS_PROXY, p(c).cast::<FURL>(), error)? };
                actor as usize
            }
            Op::ConnectionActor => unsafe { (*p(a).cast::<UNetConnection>()).actor as usize },
            Op::ConnectionActorPlayerName => {
                let actor = unsafe { (*p(a).cast::<UNetConnection>()).actor.cast::<APawn>() };
                if actor.is_null() { return Err(self.invalid("Connection has no Actor")); }
                let pri = unsafe { (*actor).player_replication_info.cast::<hp2_abi::engine::generated::APlayerReplicationInfo>() };
                if pri.is_null() { EMPTY_TEXT.as_ptr() as usize } else {
                    let text = unsafe { (*pri).player_name.data };
                    if text.is_null() { EMPTY_TEXT.as_ptr() as usize } else { text.cast::<u32>() as usize }
                }
            }
        };
        Ok(result)
    }

    unsafe fn with_pre_login<T>(
        &mut self,
        body: impl FnOnce(&mut Self, *mut std::ffi::c_void, *mut std::ffi::c_void) -> RuntimeResult<T>,
    ) -> RuntimeResult<T> {
        let mut params = AGameInfoPreLoginParams { options: empty_fstring(), address: empty_fstring(), error: empty_fstring(), fail_code: empty_fstring() };
        self.prelogin_params = &mut params;
        let result = body(self, ptr::addr_of_mut!(params.error).cast(), ptr::addr_of_mut!(params.fail_code).cast());
        self.prelogin_params = ptr::null_mut();
        let cleanup = unsafe { destroy_prelogin_strings(self.access, &mut params) };
        first_failure(result, cleanup)
    }

    unsafe fn pre_login(
        &mut self, game: *mut std::ffi::c_void, conn: *mut std::ffi::c_void,
        options: *const u32, error: *mut std::ffi::c_void, fail_code: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        let address = EMPTY_TEXT;
        let demo = self.access.native_class("Engine.DemoRecConnection")?;
        if unsafe { !hp2_core::object_model::is_a(conn.cast(), demo.cast()) } {
            return Err(self.invalid("Connection class has no canonical LowLevelGetRemoteAddress implementation"));
        }
        let params = self.prelogin_params;
        if params.is_null() { return Err(self.invalid("PreLogin FString scope is not active")); }
        unsafe {
            self.access.assign_fstring(ptr::addr_of_mut!((*params).options), std::slice::from_raw_parts(options, nul_len(options) + 1))?;
            self.access.assign_fstring(ptr::addr_of_mut!((*params).address), &address)?;
        }
        let event = unsafe { self.prelogin_event(game.cast())? };
        unsafe { crate::actor_owner::process_event(self.access, game.cast(), event, params.cast(), ptr::null_mut())?; }
        let _ = (error, fail_code);
        Ok(())
    }

    unsafe fn with_url<T>(
        &mut self, text: *const u32,
        body: impl FnOnce(&mut Self, *mut std::ffi::c_void) -> RuntimeResult<T>,
    ) -> RuntimeResult<T> {
        let defaults = crate::url_owner::UrlOwner::current_defaults(self.access)?;
        let mut storage = std::mem::MaybeUninit::<FURL>::uninit();
        unsafe { crate::url_owner::UrlOwner::construct_default(self.access, &defaults, storage.as_mut_ptr())?; }
        let url = storage.as_mut_ptr();
        if let Err(error) = unsafe { crate::url_owner::UrlOwner::parse_update(self.access, &defaults, url, ptr::null_mut(), text, 0) } {
            let _ = unsafe { crate::url_owner::UrlOwner::destroy(self.access, url) };
            return Err(error);
        }
        let result = body(self, url.cast());
        let cleanup = unsafe { crate::url_owner::UrlOwner::destroy(self.access, url) };
        first_failure(result, cleanup)
    }

    unsafe fn with_join_error<T>(
        &mut self, body: impl FnOnce(&mut Self, *mut std::ffi::c_void) -> RuntimeResult<T>,
    ) -> RuntimeResult<T> {
        let mut error = empty_fstring();
        self.join_error = &mut error;
        let result = body(self, (&mut error as *mut TArrayRaw).cast());
        self.join_error = ptr::null_mut();
        let cleanup = unsafe { self.access.destroy_owned_fstring(&mut error) };
        first_failure(result, cleanup)
    }
}

/// Keeps the nested body's failure first, matching the native scope return order.
fn first_failure<T>(result: RuntimeResult<T>, cleanup: RuntimeResult<()>) -> RuntimeResult<T> {
    match (result, cleanup) {
        (Err(error), _) => Err(error),
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(error)) => Err(error),
    }
}

const EMPTY_TEXT: [u32; 1] = [0];

fn empty_fstring() -> TArrayRaw { TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 } }
fn nul_len(mut text: *const u32) -> usize { let mut len = 0; while unsafe { *text } != 0 { len += 1; text = unsafe { text.add(1) }; } len }
fn atoi(mut text: *const u32) -> i32 {
    while matches!(unsafe { *text }, 32 | 9 | 10 | 11 | 12 | 13) { text = unsafe { text.add(1) }; }
    let negative = unsafe { *text == u32::from(b'-') };
    if negative || unsafe { *text == u32::from(b'+') } { text = unsafe { text.add(1) }; }
    let mut value = 0i32;
    while (u32::from(b'0')..=u32::from(b'9')).contains(&unsafe { *text }) { value = value.wrapping_mul(10).wrapping_add(unsafe { *text as i32 - i32::from(b'0') }); text = unsafe { text.add(1) }; }
    if negative { value.wrapping_neg() } else { value }
}
fn parse_int(text: *const u32, key: *const u32, output: *mut i32) -> bool {
    let mut value = [0u32; 128];
    // SAFETY: arguments are NUL-terminated policy text and `value` is writable.
    if !unsafe { hp2_core::text_parse::parse_string(text.cast(), key.cast(), value.as_mut_ptr().cast(), value.len() as i32) } {
        return false;
    }
    unsafe { *output = atoi(value.as_ptr()); }
    true
}

fn parse_guid(text: *const u32, key: *const u32) -> [u32; 4] {
    let mut value = [0u32; 128];
    // SAFETY: arguments are NUL-terminated policy text and `value` is writable.
    if !unsafe { hp2_core::text_parse::parse_string(text.cast(), key.cast(), value.as_mut_ptr().cast(), value.len() as i32) } {
        return [0; 4];
    }
    let mut cursor = value.as_ptr();
    let mut guid: [u32; 4] = [0; 4];
    for word in &mut guid {
        for _ in 0..8 {
            let unit = unsafe { *cursor };
            let nibble = match unit {
                c if (48..=57).contains(&c) => c - 48,
                c if (65..=70).contains(&c) => c - 65 + 10,
                c if (97..=102).contains(&c) => c - 97 + 10,
                _ => return [0; 4],
            };
            *word = (*word).wrapping_shl(4) | nibble;
            cursor = unsafe { cursor.add(1) };
        }
    }
    guid
}
unsafe fn destroy_prelogin_strings(access: CoreAccess<'_>, params: &mut AGameInfoPreLoginParams) -> RuntimeResult<()> {
    let mut first = None;
    for field in [ptr::addr_of_mut!(params.options), ptr::addr_of_mut!(params.address), ptr::addr_of_mut!(params.error), ptr::addr_of_mut!(params.fail_code)] {
        if let Err(error) = unsafe { access.destroy_owned_fstring(field) } { if first.is_none() { first = Some(error); } }
    }
    first.map_or(Ok(()), Err)
}
