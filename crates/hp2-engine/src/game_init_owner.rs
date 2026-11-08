//! Core-owned adapter for the shared GameEngine initialization policy.

use crate::game_init::{self, InitOp, InitOps};
use hp2_abi::engine::generated::{
    UObject, UEngine, UGameEngine, UClass, USDLClient, TArrayRaw, FURL, ULevel, UPlayer,
    UBoolProperty, URenderBase, URenderDevice,
};
use hp2_core::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    name_owner::FindName,
};
use std::ptr;

fn parse_init_load_slot(command_line: &[u32], prefix: &[u32]) -> Option<i32> {
    const QUOTE: u32 = b'"' as u32;
    fn separator(unit: u32) -> bool { matches!(unit, 0 | 0x20 | 0x09 | 0x0d | 0x0a | 0x2c) }
    let end = command_line.iter().position(|unit| *unit == 0).unwrap_or(command_line.len());
    let command_line = &command_line[..end];
    let mut pos = 0;
    while pos < command_line.len() {
        while pos < command_line.len() && separator(command_line[pos]) { pos += 1; }
        if pos == command_line.len() { break; }
        let quoted = command_line[pos] == QUOTE;
        let token = pos + usize::from(quoted);
        let token_end;
        if quoted {
            let relative = command_line[token..].iter().position(|unit| *unit == QUOTE)?;
            token_end = token + relative;
            pos = token_end + 1;
        } else {
            token_end = token + command_line[token..].iter().position(|unit| separator(*unit)).unwrap_or(command_line.len() - token);
            pos = token_end;
        }
        if token_end - token < prefix.len()
            || !command_line[token..token + prefix.len()].iter().zip(prefix).all(|(&left, &right)| {
                char::from_u32(left).unwrap_or('\0').eq_ignore_ascii_case(&char::from_u32(right).unwrap_or('\0'))
            })
        { continue; }
        let mut value = token + prefix.len();
        let mut value_end = token_end;
        if value < value_end && command_line[value] == QUOTE {
            if command_line[value_end - 1] != QUOTE { return None; }
            value += 1;
            value_end -= 1;
        }
        if value == value_end { return None; }
        let mut parsed = 0i32;
        for &unit in &command_line[value..value_end] {
            if !(b'0' as u32..=b'9' as u32).contains(&unit) { return None; }
            let digit = (unit - b'0' as u32) as i32;
            if parsed > (i32::MAX - digit) / 10 { return None; }
            parsed = parsed * 10 + digit;
        }
        return Some(parsed);
    }
    None
}

pub(crate) fn system_path(access: CoreAccess<'_>, slot_path: bool) -> RuntimeResult<std::path::PathBuf> {
    let system = access.system()?;
    let raw = unsafe { if slot_path { ptr::addr_of!((*system).save_slot_path).read_unaligned() } else { ptr::addr_of!((*system).save_path).read_unaligned() } };
    if raw.num < 0 || (raw.num > 0 && raw.data.is_null()) {
        return Err(RuntimeError::new(if slot_path { "Invalid canonical SaveSlotPath" } else { "Invalid canonical SavePath" }));
    }
    let text: String = if raw.num > 0 {
        unsafe { std::slice::from_raw_parts(raw.data.cast::<u32>(), raw.num as usize) }
            .iter().copied().take_while(|unit| *unit != 0)
            .map(|unit| char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER)).collect()
    } else { String::new() };
    let path = std::path::PathBuf::from(text);
    Ok(if path.is_absolute() { path } else { access.data_root().join("System").join(path) })
}
fn wide(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).chain(std::iter::once(0)).collect()
}
fn copy_text(destination: &mut [u32], source: &[u32]) -> RuntimeResult<()> {
    let source = &source[..source.iter().position(|unit| *unit == 0).unwrap_or(source.len())];
    if source.len() >= destination.len() {
        return Err(RuntimeError::new("GameEngine Init text exceeds destination capacity"));
    }
    destination.fill(0);
    destination[..source.len()].copy_from_slice(source);
    Ok(())
}

fn command_line_value(command_line: &[u32], prefix: &str) -> Option<Vec<u32>> {
    let prefix: Vec<u32> = prefix.chars().map(|character| u32::from(character.to_ascii_uppercase())).collect();
    let end = command_line.iter().position(|unit| *unit == 0).unwrap_or(command_line.len());
    let command_line = &command_line[..end];
    let start = command_line.windows(prefix.len()).position(|window| {
        window.iter().zip(&prefix).all(|(&unit, &expected)| {
            char::from_u32(unit).is_some_and(|character| u32::from(character.to_ascii_uppercase()) == expected)
        })
    })? + prefix.len();
    let value = command_line[start..].iter().copied().take_while(|unit| *unit != 0 && !char::from_u32(*unit).is_some_and(char::is_whitespace));
    Some(value.chain(std::iter::once(0)).collect())
}

fn equal_text_ignore_case(left: &[u32], right: &[u32]) -> bool {
    let left_len = left.iter().position(|unit| *unit == 0).unwrap_or(left.len());
    let right_len = right.iter().position(|unit| *unit == 0).unwrap_or(right.len());
    left_len == right_len && left[..left_len].iter().zip(&right[..right_len]).all(|(&a, &b)| {
        char::from_u32(a).unwrap_or('\0').eq_ignore_ascii_case(&char::from_u32(b).unwrap_or('\0'))
    })
}

struct CoreInitOps<'a> {
    access: CoreAccess<'a>,
    engine: *mut UGameEngine,
    client_class: *mut UClass,
    audio_class: *mut UClass,
    render_class: *mut UClass,
    console_class: *mut UClass,
    viewport: *mut hp2_abi::engine::generated::USDLViewport,
    replay_file: TArrayRaw,
    entry_url: FURL,
    default_url: FURL,
    url: FURL,
    error: TArrayRaw,
    fallback_url: FURL,
    play_error: TArrayRaw,
    cursor: *const u32,
    parm: [u32; 4096],
    file_token: [u32; 4096],
    load_url: [u32; 32],
    load_game_slot: i32,
    last_action_result: bool,
    entry_url_live: bool,
    default_url_live: bool,
    url_live: bool,
    error_live: bool,
    play_error_live: bool,
    replay_file_live: bool,
    fallback_url_live: bool,
}

impl CoreInitOps<'_> {
    fn default_local_map(&self) -> RuntimeResult<Vec<u32>> {
        let section = wide("URL");
        let key = wide("LocalMap");
        let mut map = self.access.config_get_units(&section, &key)?.unwrap_or_else(|| vec![0]);
        if map.last() != Some(&0) {
            map.push(0);
        }
        Ok(map)
    }
    fn parse_replay_argument(&mut self, prefix: &str) -> RuntimeResult<i32> {
        let Some(argument) = command_line_value(self.access.command_line(), prefix) else {
            return Ok(0);
        };
        unsafe { self.access.assign_fstring(&mut self.replay_file, &argument)?; }
        Ok(1)
    }
    fn cleanup(&mut self) -> RuntimeResult<()> {
        let mut first = None;
        if self.play_error_live {
            if let Err(error) = unsafe { self.access.destroy_owned_fstring(&mut self.play_error) } {
                first = Some(error);
            }
            self.play_error_live = false;
        }
        if self.url_live {
            if let Err(error) = unsafe { crate::url_owner::UrlOwner::destroy(self.access, &mut self.url) } {
                if first.is_none() { first = Some(error); }
            }
            self.url_live = false;
        }
        if self.fallback_url_live {
            if let Err(error) = unsafe { crate::url_owner::UrlOwner::destroy(self.access, &mut self.fallback_url) } {
                if first.is_none() { first = Some(error); }
            }
            self.fallback_url_live = false;
        }
        if self.replay_file_live {
            if let Err(error) = unsafe { self.access.destroy_owned_fstring(&mut self.replay_file) } {
                if first.is_none() { first = Some(error); }
            }
            self.replay_file_live = false;
        }
        if self.default_url_live {
            if let Err(error) = unsafe { crate::url_owner::UrlOwner::destroy(self.access, &mut self.default_url) } {
                if first.is_none() { first = Some(error); }
            }
            self.default_url_live = false;
        }
        if self.entry_url_live {
            if let Err(error) = unsafe { crate::url_owner::UrlOwner::destroy(self.access, &mut self.entry_url) } {
                if first.is_none() { first = Some(error); }
            }
            self.entry_url_live = false;
        }
        if self.error_live {
            if let Err(error) = unsafe { self.access.destroy_owned_fstring(&mut self.error) } {
                if first.is_none() { first = Some(error); }
            }
            self.error_live = false;
        }
        first.map_or(Ok(()), Err)
    }
}
impl InitOps for CoreInitOps<'_> {
    type Error = RuntimeError;

    fn value(&mut self, op: InitOp) -> RuntimeResult<i32> {
        use InitOp::*;

        match op {
            CleanFileCache => {
                unsafe { crate::file_cache_owner::clean_file_cache(self.access)?; }
                Ok(0)
            }
            IsClient => Ok(i32::from(self.access.process_mode().is_client)),
            HasClient => Ok(i32::from(unsafe {
                !(*self.engine.cast::<hp2_abi::engine::generated::UEngine>()).client.is_null()
            })),
            HasAudio => Ok(i32::from(unsafe {
                !(*self.engine.cast::<hp2_abi::engine::generated::UEngine>()).audio.is_null()
            })),
            CheckSize => {
                let object = unsafe { &*self.engine.cast::<UObject>() };
                if object.class.is_null()
                    || unsafe { (*object.class.cast::<UClass>()).properties_size as usize }
                        != std::mem::size_of::<UGameEngine>()
                {
                    return Err(RuntimeError::new("UGameEngine class properties size mismatch"));
                }
                Ok(0)
            }
            BaseInit => {
                let engine = std::ptr::NonNull::new(self.engine.cast::<hp2_abi::engine::generated::UEngine>())
                    .ok_or_else(|| RuntimeError::new("Null canonical UEngine"))?;
                unsafe { crate::engine_owner::init(self.access, engine)?; }
                Ok(0)
            }
            ClearLevel => { unsafe { (*self.engine).g_level = std::ptr::null_mut(); } Ok(0) }
            ClearViewport => { unsafe { (*self.engine).g_viewport = std::ptr::null_mut(); } Ok(0) }
            ClearDrawing => { unsafe { (*self.engine).g_is_drawing = false; } Ok(0) }
            LoadClientClass => {
                let base = self.access.native_class("Engine.Client")?;
                self.client_class = unsafe {
                    self.access.static_load_class(base, ptr::null_mut(), &wide("ini:Engine.Engine.ViewportManager"), 1)?
                };
                Ok(0)
            }
            ConstructClient => {
                let client = unsafe {
                    self.access.construct_object(self.client_class, self.access.transient_package(), 0, 0, ptr::null_mut())?
                };
                unsafe { (*self.engine.cast::<UEngine>()).client = client.cast(); }
                Ok(0)
            }
            InitClient => {
                let client = unsafe { (*self.engine.cast::<UEngine>()).client.cast::<USDLClient>() };
                unsafe { crate::platform_owner::init_client(self.access, client, self.engine.cast())?; }
                Ok(0)
            }
            LoadRenderClass => {
                let base = self.access.native_class("Engine.RenderBase")?;
                self.render_class = unsafe {
                    self.access.static_load_class(base, ptr::null_mut(), &wide("ini:Engine.Engine.Render"), 1)?
                };
                Ok(0)
            }
            ConstructRender => {
                let render = unsafe {
                    self.access.construct_object(self.render_class, self.access.transient_package(), 0, 0, ptr::null_mut())?
                };
                unsafe { (*self.engine.cast::<UEngine>()).render = render.cast(); }
                Ok(0)
            }
            InitRender => {
                let render = unsafe { (*self.engine.cast::<UEngine>()).render };
                let class = self.access.native_class("Render.Render")?;
                if !unsafe { hp2_core::object_model::is_a(render.cast(), class.cast()) } {
                    return Err(RuntimeError::new("Canonical GameEngine requires the Rust Render implementation"));
                }
                unsafe { crate::render_owner::initialize(self.access, render.cast(), self.engine.cast())?; }
                Ok(0)
            }
            ConstructError => {
                unsafe { self.access.assign_fstring(&mut self.error, &[0])?; }
                self.error_live = true;
                Ok(0)
            }
            ConstructPlayError => {
                unsafe { self.access.assign_fstring(&mut self.play_error, &[0])?; }
                self.play_error_live = true;
                Ok(0)
            }
            ConstructEntryUrl => {
                let defaults = crate::url_owner::UrlOwner::current_defaults(self.access)?;
                unsafe {
                    crate::url_owner::UrlOwner::construct_local_filename(
                        self.access, &defaults, &mut self.entry_url, Some(&[69,110,116,114,121,0]),
                    )?;
                    self.entry_url_live = true;
                }
                Ok(0)
            }
            LoadEntry => {
                let loaded = unsafe {
                    !crate::game_load_map_owner::load_map(
                        self.access, self.engine, &self.entry_url, ptr::null_mut(), ptr::null(), &mut self.error,
                    )?.is_null()
                };
                Ok(i32::from(loaded))
            }
            DestroyEntryUrl => {
                if self.entry_url_live {
                    unsafe { crate::url_owner::UrlOwner::destroy(self.access, &mut self.entry_url)?; }
                    self.entry_url_live = false;
                }
                Ok(0)
            }
            ExchangeEntry => {
                unsafe { std::mem::swap(&mut (*self.engine).g_level, &mut (*self.engine).g_entry); }
                Ok(0)
            }
            ConstructDefaultUrl => {
                let defaults = crate::url_owner::UrlOwner::current_defaults(self.access)?;
                unsafe {
                    crate::url_owner::UrlOwner::construct_default(self.access, &defaults, &mut self.default_url)?;
                }
                self.default_url_live = true;
                Ok(0)
            }
            LoadDefaultConfig => {
                let defaults = crate::url_owner::UrlOwner::current_defaults(self.access)?;
                let section = wide("DefaultPlayer");
                let filename = wide("User");
                unsafe {
                    crate::url_owner::UrlOwner::load_config(
                        self.access, &defaults, &mut self.default_url, section.as_ptr(), filename.as_ptr(),
                    )?;
                }
                Ok(0)
            }
            ConstructUrl => {
                let defaults = crate::url_owner::UrlOwner::current_defaults(self.access)?;
                unsafe {
                    crate::url_owner::UrlOwner::construct_default(self.access, &defaults, &mut self.url)?;
                    self.url_live = true;
                    crate::url_owner::UrlOwner::parse_update(
                        self.access, &defaults, &mut self.url, &mut self.default_url, self.parm.as_ptr(), 1,
                    )?;
                }
                Ok(0)
            }
            IsValidUrl => Ok(i32::from(unsafe { self.url.valid != 0 })),
            IsErrorEmpty => Ok(i32::from(self.error.num <= 1)),
            IsDefaultMap => {
                let local = self.default_local_map()?;
                Ok(i32::from(equal_text_ignore_case(&self.parm, &local)))
            }
            IsOptionToken => Ok(i32::from(self.parm[0] == u32::from(b'-'))),
            HasProtocol => Ok(i32::from(self.parm.windows(3).any(|part| part == [58, 47, 47]))),
            IsOptionUrl => Ok(i32::from(self.parm[0] == u32::from(b'?'))),
            InitCommandLine => {
                self.parm.fill(0);
                self.cursor = self.access.command_line().as_ptr();
                Ok(0)
            }
            ParseToken => {
                self.parm.fill(0);
                Ok(i32::from(unsafe {
                    {
                        let mut cursor: *const i32 = self.cursor.cast();
                        let parsed = hp2_core::tokenize::parse_token(
                            &mut cursor,
                            self.parm.as_mut_ptr().cast(),
                            self.parm.len() as i32,
                            false,
                        );
                        self.cursor = cursor.cast();
                        parsed
                    }
                }))
            }
            IsServerToken => {
                let token: Vec<u32> = self.parm.iter().copied().take_while(|unit| *unit != 0)
                    .map(|unit| char::from_u32(unit).unwrap_or('\0').to_ascii_uppercase() as u32).collect();
                Ok(i32::from(token == [83, 69, 82, 86, 69, 82]))
            }
            CopyDefaultMap => {
                let map = self.default_local_map()?;
                copy_text(&mut self.parm, &map).map(|()| 0)
            }
            ConstructReplayFile => {
                unsafe { self.access.assign_fstring(&mut self.replay_file, &[0])?; }
                self.replay_file_live = true;
                Ok(0)
            }
            ParseReplay => self.parse_replay_argument("-REPLAY="),
            ParseRecord => self.parse_replay_argument("-RECORD="),
            StartReplay => {
                let replay = unsafe { &mut (*self.engine).replay };
                Ok(i32::from(unsafe {
                    crate::replay_owner::open_file(
                        self.access, replay, self.replay_file.data.cast(), false, ptr::null(),
                    )?
                }))
            }
            StartRecord => {
                let replay = unsafe { &mut (*self.engine).replay };
                Ok(i32::from(unsafe {
                    crate::replay_owner::open_file(
                        self.access, replay, self.replay_file.data.cast(), true, self.parm.as_ptr(),
                    )?
                }))
            }
            CopyReplayUrl => {
                let url = unsafe { (*self.engine).replay.url_str };
                if url.num < 0 || (url.num > 0 && url.data.is_null()) {
                    return Err(RuntimeError::new("Invalid canonical replay URL storage"));
                }
                let units = if url.num == 0 { &[] } else {
                    unsafe { std::slice::from_raw_parts(url.data.cast::<u32>(), url.num as usize) }
                };
                copy_text(&mut self.parm, units)?;
                Ok(0)
            }
            ConvertFileToken => Ok(i32::from(unsafe {
                crate::url::file_path_to_token(self.parm.as_ptr().cast(), self.file_token.as_mut_ptr().cast(), self.file_token.len() as i32)
            })),
            CopyFileToken => {
                let end = self.file_token.iter().position(|unit| *unit == 0).unwrap_or(self.file_token.len());
                copy_text(&mut self.parm, &self.file_token[..end])?;
                Ok(0)
            }
            NewViewport => {
                let client = unsafe { (*self.engine.cast::<UEngine>()).client.cast::<USDLClient>() };
                self.viewport = unsafe { crate::platform_owner::new_viewport(self.access, client, 0)? };
                unsafe { (*self.engine.cast::<hp2_abi::engine::generated::UGameEngine>()).g_viewport = self.viewport.cast(); }
                Ok(0)
            }
            LoadConsoleClass => {
                let base = self.access.native_class("Engine.Console")?;
                self.console_class = unsafe {
                    self.access.static_load_class(base, ptr::null_mut(), &wide("ini:Engine.Engine.Console"), 1)?
                };
                Ok(0)
            }
            ConstructConsole => {
                let console = unsafe {
                    self.access.construct_object(self.console_class, self.viewport.cast(), 0, 0, ptr::null_mut())?
                };
                unsafe { (*self.viewport).base.console = console.cast(); }
                Ok(0)
            }
            InitConsole => {
                let console = unsafe { (*self.viewport).base.console.cast::<hp2_abi::engine::generated::UConsole>() };
                unsafe { crate::console_owner::initialize(self.access, console, self.viewport.cast())?; }
                Ok(0)
            }
            OpenWindow => {
                unsafe { crate::platform_owner::open_viewport_window(self.access, self.viewport, false, -1, -1, -1, -1)?; }
                Ok(0)
            }
            SetAudioViewport => {
                let audio = unsafe { (*self.engine.cast::<UEngine>()).audio };
                unsafe { crate::audio_owner::set_viewport(self.access, audio.cast(), self.viewport.cast())?; }
                Ok(0)
            }
            Browse => {
                self.last_action_result = unsafe {
                    crate::game_travel_owner::browse(
                        self.access, self.engine, &self.url, ptr::null(), &mut self.error,
                    )?
                };
                Ok(i32::from(self.last_action_result))
            }
            BrowseFallback => {
                let defaults = crate::url_owner::UrlOwner::current_defaults(self.access)?;
                let map = self.default_local_map()?;
                unsafe {
                    crate::url_owner::UrlOwner::construct_default(self.access, &defaults, &mut self.fallback_url)?;
                    self.fallback_url_live = true;
                    crate::url_owner::UrlOwner::parse_update(
                        self.access, &defaults, &mut self.fallback_url, ptr::addr_of_mut!(self.default_url), map.as_ptr(), 1,
                    )?;
                    self.last_action_result = crate::game_travel_owner::browse(
                        self.access, self.engine, &self.fallback_url, ptr::null(), &mut self.error,
                    )?;
                }
                Ok(i32::from(self.last_action_result))
            }
            InvalidUrl => Err(RuntimeError::new("Invalid startup URL")),
            FailedBrowse => Err(RuntimeError::new("Failed to browse startup URL")),
            FailedEntry => Err(RuntimeError::new("Failed to browse Entry map")),
            InvalidLoadSlot => Err(RuntimeError::new("Invalid startup load slot")),
            InitInputSystem => {
                unsafe { crate::input_owner::static_init_input(self.access)?; }
                Ok(0)
            }
            InitViewportInput => {
                let input = unsafe { (*self.viewport).base.input.cast::<hp2_abi::engine::generated::UInput>() };
                unsafe { crate::input_owner::init(self.access, input, self.viewport.cast())?; }
                Ok(0)
            }
            InitAudio => {
                let engine = self.engine.cast::<UEngine>();
                let enabled = unsafe { (*engine).use_sound != 0 }
                    && self.access.process_mode().is_client
                    && !unsafe {
                        hp2_core::search::parse_param(
                            self.access.command_line().as_ptr().cast(),
                            wide("NOSOUND").as_ptr().cast(),
                        )
                    };
                if enabled {
                    let base = self.access.native_class("Engine.AudioSubsystem")?;
                    self.audio_class = unsafe {
                        self.access.static_load_class(
                            base, ptr::null_mut(), &wide("ini:Engine.Engine.AudioDevice"), 1,
                        )?
                    };
                    let audio = unsafe {
                        self.access.construct_object(self.audio_class, self.access.transient_package(), 0, 0, ptr::null_mut())?
                    };
                    unsafe { (*engine).audio = audio.cast(); }
                    if !crate::audio_owner::initialize(self.access, audio.cast(), self.viewport.cast())? {
                        self.access.emit_log(self.access.intern_str("Init", FindName::Add)?, &wide("Audio initialization failed"))?;
                        unsafe { self.access.conditional_destroy(audio)?; }
                        unsafe { (*engine).audio = ptr::null_mut(); }
                    }
                }
                Ok(0)
            }
            DestroyPlayError => {
                if self.play_error_live {
                    unsafe { self.access.destroy_owned_fstring(&mut self.play_error)?; }
                    self.play_error_live = false;
                }
                Ok(0)
            }
            LogInitialized => {
                let category = self.access.intern_str("Init", FindName::Add)?;
                self.access.emit_log(category, &wide("Game engine initialized"))?;
                Ok(0)
            }
            FormatLoadUrl => Ok(i32::from(unsafe {
                crate::game_save::format_load_game_url(self.load_game_slot, self.load_url.as_mut_ptr(), self.load_url.len() as i32)
            })),
            DetailChange => {
                if self.viewport.is_null() {
                    return Err(RuntimeError::new("GameEngine Init DetailChange has no viewport"));
                }
                let render = unsafe { (*self.viewport).base.ren_dev.cast::<URenderDevice>() };
                if render.is_null() {
                    return Err(RuntimeError::new("GameEngine Init DetailChange has no render device"));
                }
                let detail = unsafe { (*render).high_detail_actors() };
                let level = unsafe { (*self.engine).g_level.cast::<ULevel>() };
                unsafe {
                    crate::level::level_detail_change_with(
                        &mut CoreDetailChangeOps(self.access),
                        level.cast(),
                        i32::from(detail),
                    )?;
                }
                Ok(0)
            }
            SpawnPlayActor => {
                let level = unsafe { (*self.engine).g_level.cast::<ULevel>() };
                let player = self.viewport.cast::<UPlayer>();
                let actor = unsafe {
                    crate::level_player_owner::spawn_play_actor(
                        self.access, level, player, 2, &mut self.url, &mut self.play_error,
                    )?
                };
                Ok(i32::from(!actor.is_null()))
            }
            FailedPlayActor => {
                let message = unsafe { hp2_core::native_text::fstring_units(&self.play_error) };
                Err(RuntimeError::from_utf32(message.iter().map(|&unit| unit as u32).collect()))
            }
            ServerTravel => {
                let level = unsafe { (*self.engine).g_level.cast::<ULevel>() };
                unsafe { crate::game_exec_owner::server_travel_event(self.access, level, self.load_url.as_ptr(), 0)?; }
                Ok(0)
            }
            ConsumeLoadSlot => {
                let state = self.access.owner_context::<crate::core_owner::EngineState>()
                    .ok_or_else(|| RuntimeError::new("Canonical Engine state is unavailable"))?;
                if state.load_slot_consumed.get() { return Ok(0); }
                let repair_prefix = wide("-REPAIRSAVE=");
                let load_prefix = wide("-LOAD=");
                let repair = parse_init_load_slot(self.access.command_line(), &repair_prefix[..repair_prefix.len() - 1]);
                let slot = repair.or_else(|| parse_init_load_slot(self.access.command_line(), &load_prefix[..load_prefix.len() - 1]));
                if let Some(slot) = slot {
                    state.load_slot_consumed.set(true);
                    state.repair_save_slot.set(if repair.is_some() { slot } else { -1 });
                    self.load_game_slot = slot;
                    Ok(1)
                } else { Ok(0) }
            }
            MakeSaveDirectory | MakeSaveSlotDirectory => {
                let path = system_path(self.access, op == MakeSaveSlotDirectory)?;
                let _ = std::fs::create_dir(&path);
                Ok(0)
            }
            LogLoadSlot => {
                let category = self.access.intern_str("Log", FindName::Add)?;
                self.access.emit_log(category, &wide(&format!("Loading game slot {}", self.load_game_slot)))?;
                Ok(0)
            }
        }
    }
}

struct CoreDetailChangeOps<'a>(CoreAccess<'a>);

impl crate::level::DetailChangeOps for CoreDetailChangeOps<'_> {
    type Error = RuntimeError;

    fn set_high_detail(&mut self, level: *mut std::ffi::c_void, value: i32) -> RuntimeResult<()> {
        let level = level.cast::<ULevel>();
        let info = unsafe { crate::package_map_level_owner::level_info(level)? };
        let name = self.0.intern_str("bHighDetailMode", FindName::Find)?;
        let bool_class = self.0.native_class("BoolProperty")?;
        let property = unsafe {
            self.0.find_field((*info.cast::<UObject>()).class.cast(), name, bool_class)
        }.cast::<hp2_abi::engine::generated::UProperty>();
        if property.is_null() {
            return Err(RuntimeError::new("LevelInfo bHighDetailMode property is missing"));
        }
        let mask = unsafe { (*property.cast::<UBoolProperty>()).bit_mask as u32 };
        if mask == 0 {
            return Err(RuntimeError::new("LevelInfo bHighDetailMode has an empty bit mask"));
        }
        let data = unsafe { self.0.property_address(info.cast(), property)? }.cast::<u32>();
        let old = unsafe { data.read_unaligned() };
        unsafe { data.write_unaligned(if value != 0 { old | mask } else { old & !mask }); }
        Ok(())
    }

    fn has_game(&mut self, level: *mut std::ffi::c_void) -> RuntimeResult<bool> {
        let info = unsafe { crate::package_map_level_owner::level_info(level.cast())? };
        Ok(!unsafe { (*info).game }.is_null())
    }

    fn event_detail_change(&mut self, level: *mut std::ffi::c_void) -> RuntimeResult<()> {
        let info = unsafe { crate::package_map_level_owner::level_info(level.cast())? };
        let game = unsafe { (*info).game.cast::<hp2_abi::engine::generated::AActor>() };
        if game.is_null() {
            return Err(RuntimeError::new("LevelInfo has_game disagrees with GameInfo"));
        }
        let name = self.0.intern_str("DetailChange", FindName::Find)?;
        let function = unsafe { self.0.find_object_function_by_name(game.cast(), name, false)? };
        if function.is_null() {
            return Err(RuntimeError::new("GameInfo DetailChange event is missing"));
        }
        unsafe {
            crate::actor_owner::process_event(self.0, game, function, ptr::null_mut(), ptr::null_mut())
        }
    }
}

/// Runs the shared Level::DetailChange kernel against canonical Core records.
///
/// # Safety
/// `level` is a live canonical ULevel owned by `access`.
pub unsafe fn level_detail_change(access: CoreAccess<'_>, level: *mut ULevel, high_detail: i32) -> RuntimeResult<()> {
    if level.is_null() { return Err(RuntimeError::new("Null canonical ULevel in DetailChange")); }
    unsafe {
        crate::level::level_detail_change_with(
            &mut CoreDetailChangeOps(access),
            level.cast(),
            high_detail,
        )
    }
}

pub unsafe fn set_level_high_detail(access: CoreAccess<'_>, level: *mut ULevel, value: i32) -> RuntimeResult<()> {
    if level.is_null() { return Err(RuntimeError::new("Null canonical ULevel in high-detail update")); }
    crate::level::DetailChangeOps::set_high_detail(&mut CoreDetailChangeOps(access), level.cast(), value)
}

pub unsafe fn level_high_detail(access: CoreAccess<'_>, level: *mut ULevel) -> RuntimeResult<i32> {
    if level.is_null() { return Err(RuntimeError::new("Null canonical ULevel while reading high-detail mode")); }
    let info = unsafe { crate::package_map_level_owner::level_info(level)? };
    let name = access.intern_str("bHighDetailMode", FindName::Find)?;
    let bool_class = access.native_class("BoolProperty")?;
    let property = unsafe { access.find_field((*info.cast::<UObject>()).class.cast(), name, bool_class) };
    if property.is_null() { return Err(RuntimeError::new("LevelInfo bHighDetailMode property is missing")); }
    let mask = unsafe { (*property.cast::<UBoolProperty>()).bit_mask };
    if mask == 0 { return Err(RuntimeError::new("LevelInfo bHighDetailMode has an empty bit mask")); }
    let data = unsafe { access.property_address(info.cast(), property.cast())?.cast::<u32>() };
    Ok(i32::from(unsafe { data.read_unaligned() } & mask != 0))
}

/// Runs the shared source-order initialization policy on a canonical GameEngine.
///
/// # Safety
/// `engine` must identify a live Core-owned `UGameEngine`.
pub unsafe fn init(access: CoreAccess<'_>, engine: *mut UGameEngine) -> RuntimeResult<()> {
    if engine.is_null() {
        return Err(RuntimeError::new("Null canonical UGameEngine"));
    }
    // SAFETY: zero is a valid inactive representation for the ABI storage;
    // the URL/FString owners are only used after their explicit construction ops.
    let mut ops = unsafe {
        CoreInitOps {
            access,
            engine,
            client_class: ptr::null_mut(),
            audio_class: ptr::null_mut(),
            render_class: ptr::null_mut(),
            console_class: ptr::null_mut(),
            viewport: ptr::null_mut(),
            error: std::mem::zeroed(),
            entry_url: std::mem::zeroed(),
            default_url: std::mem::zeroed(),
            url: std::mem::zeroed(),
            fallback_url: std::mem::zeroed(),
            replay_file: std::mem::zeroed(),
            play_error: std::mem::zeroed(),
            cursor: ptr::null(),
            parm: [0; 4096],
            file_token: [0; 4096],
            load_url: [0; 32],
            load_game_slot: 0,
            last_action_result: false,
            entry_url_live: false,
            default_url_live: false,
            url_live: false,
            error_live: false,
            play_error_live: false,
            replay_file_live: false,
            fallback_url_live: false,
        }
    };
    let result = game_init::init_with(&mut ops);
    let cleanup = ops.cleanup();
    match result {
        Err(error) => {
            let _ = cleanup;
            Err(error)
        }
        Ok(()) => cleanup,
    }
}
