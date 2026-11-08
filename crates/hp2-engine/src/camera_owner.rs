//! Canonical viewport operations used by player handoff and camera ownership.
use std::ffi::c_void;
use hp2_abi::engine::generated::{AActor, ALevelInfo, APlayerPawn, ULevel, UPlayer, UViewport, UNetConnection, UNetDriver, UTextBuffer, UControlChannel};
use hp2_abi::FColor;
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};

/// Executes the canonical source-ordered `UViewport::Exec` dispatch chain.
///
/// # Safety
/// `viewport` must be a live canonical viewport; every referenced subobject must remain live
/// through the synchronous dispatch.
pub(crate) unsafe fn exec(
    access: CoreAccess<'_>,
    viewport: *mut UViewport,
    command: *const u32,
    output: &mut crate::exec_output::ExecOutput<'_>,
) -> RuntimeResult<bool> {
    if viewport.is_null() || command.is_null() {
        return Err(RuntimeError::new("Invalid canonical UViewport Exec arguments"));
    }
    if unsafe { (*viewport).actor.is_null() } {
        return Err(RuntimeError::new("UViewport::Exec requires an actor"));
    }
    let mut context = ExecContext { access, output };
    unsafe {
        crate::camera_exec::viewport_exec_dispatch_with(
            &mut context,
            viewport,
            command,
            |context, input, command| {
                crate::input_owner::exec(context.access, input.cast(), command, context.output)
            },
            |context, device, command| {
                crate::render_device_owner::exec(context.access, device.cast(), command, context.output)
            },
            |context, viewport, command| viewport_commands(context, viewport, command),
            |context, viewport, command| {
                let console = unsafe { (*viewport).console };
                if console.is_null() { return Ok(false); }
                let actor = unsafe { (*viewport).actor };
                unsafe {
                    crate::engine_exec_owner::process_console_exec(
                        context.access,
                        console.cast(),
                        actor.cast(),
                        command,
                        context.output,
                    )
                }
            },
            |context, viewport, command| {
                unsafe { crate::player_exec_owner::exec(context.access, viewport.cast::<UPlayer>(), command, context.output) }
            },
        )
    }
}

struct ExecContext<'access, 'borrow, 'sink> {
    access: CoreAccess<'access>,
    output: &'borrow mut crate::exec_output::ExecOutput<'sink>,
}

fn viewport_commands(
    context: &mut ExecContext<'_, '_, '_>,
    viewport: *mut UViewport,
    command: *const u32,
) -> RuntimeResult<bool> {
    let actor = unsafe { (*viewport).actor.cast::<APlayerPawn>() };
    unsafe {
        crate::camera_exec::viewport_commands_with(
            command.cast(),
            |cursor, token| Ok(match_command(cursor, token)),
            |branch, cursor| {
                let mut tail = cursor.cast::<u32>();
                match branch {
                crate::camera_exec::ViewportCommand::GetPing => {
                    (context.output)(&[48])?;
                    Ok(true)
                }
                crate::camera_exec::ViewportCommand::Inject => {
                    let level = unsafe { (*actor).base.base.x_level.cast::<ULevel>() };
                    if level.is_null() { return Err(RuntimeError::new("INJECT actor has no level")); }
                    let driver = unsafe { (*level).base.net_driver.cast::<UNetDriver>() };
                    if driver.is_null() { return Ok(true); }
                    let connection = unsafe { (*driver).server_connection.cast::<UNetConnection>() };
                    if connection.is_null() { return Ok(true); }
                    let mut message = vec![73, 110, 106, 101, 99, 116, 105, 110, 103, 58, 32];
                    let mut unit = cursor.cast::<u32>();
                    while unsafe { *unit } != 0 {
                        message.push(unsafe { *unit });
                        unit = unsafe { unit.add(1) };
                    }
                    (context.output)(&message)?;
                    let formatted = format_logf_without_arguments(cursor.cast())?;
                    unsafe { send_control_text(context.access, connection, formatted.as_ptr())?; }
                    Ok(true)
                }
                crate::camera_exec::ViewportCommand::NetSpeed
                | crate::camera_exec::ViewportCommand::LanSpeed => {
                    let want_lan = matches!(branch, crate::camera_exec::ViewportCommand::LanSpeed);
                    let rate = unsafe { hp2_core::ansi::atoi_checked(cursor.cast()).unwrap_or(0) };
                    require_pawn(actor)?;
                    let level = unsafe { (*actor).base.base.x_level.cast::<ULevel>() };
                    if level.is_null() { return Err(RuntimeError::new("NETSPEED actor has no level")); }
                    let driver = unsafe { (*level).base.net_driver.cast::<UNetDriver>() };
                    let player_class = context.access.native_class("Engine.Player")?;
                    let defaults = unsafe { context.access.default_object(player_class)? }.cast::<UPlayer>();
                    unsafe {
                        crate::camera_exec::net_speed_with(
                            rate,
                            want_lan,
                            !driver.is_null(),
                            || {
                                (*defaults).configured_internet_speed = rate;
                                unsafe { context.access.save_config(defaults.cast(), 0x4000, None) }
                            },
                            || {
                                let driver = unsafe { driver.cast::<UNetDriver>() };
                                Ok(unsafe { ((*driver).server_connection.cast(), (*driver).max_client_rate) })
                            },
                            |connection| {
                                let url_defaults = crate::url_owner::UrlOwner::current_defaults(context.access)?;
                                let lan_key = [76, 65, 78, 0];
                                unsafe {
                                    crate::url_owner::UrlOwner::has_option(
                                        context.access,
                                        &url_defaults,
                                        std::ptr::addr_of!((*connection.cast::<UNetConnection>()).url),
                                        lan_key.as_ptr(),
                                    )
                                }
                            },
                            |connection, speed| {
                                unsafe {
                                    (*connection.cast::<UNetConnection>()).current_net_speed = speed;
                                    (*viewport).current_net_speed = speed;
                                }
                                Ok(())
                            },
                            |connection, rate| {
                                let text = format!("NETSPEED {rate}");
                                let mut units = text.chars().map(u32::from).collect::<Vec<_>>();
                                units.push(0);
                                unsafe { send_control_text(context.access, connection.cast(), units.as_ptr()) }
                            },
                        )?;
                    }
                    Ok(true)
                }
                crate::camera_exec::ViewportCommand::ShowAll => {
                    require_pawn(actor)?;
                    let level = unsafe { (*actor).base.base.x_level.cast::<ULevel>() };
                    if level.is_null() { return Err(RuntimeError::new("SHOWALL has no actor level")); }
                    let actors = unsafe { (*level).base.actors };
                    if actors.array_num < 0 || actors.array_num > actors.array_max
                        || (actors.array_num > 0 && actors.data.is_null())
                    {
                        return Err(RuntimeError::new("SHOWALL level actor array is invalid"));
                    }
                    for index in 0..actors.array_num as usize {
                        let target = unsafe { actors.data.cast::<*mut AActor>().add(index).read_unaligned() };
                        if !target.is_null() { unsafe { (*target).set_b_hidden(false); } }
                    }
                    Ok(true)
                }
                crate::camera_exec::ViewportCommand::Report => {
                    unsafe { report_viewport(context, viewport, cursor.cast())?; }
                    Ok(true)
                }
                crate::camera_exec::ViewportCommand::Shot => {
                    let filename = unsafe { parse_camera_token(&mut tail, 512) };
                    let path = match filename {
                        Some(name) => Some(save_slot_path(context.access, &name)?),
                        None => unique_shot_path()?,
                    };
                    let Some(path) = path else { return Ok(true); };
                    let width = unsafe { (*viewport).size_x };
                    let height = unsafe { (*viewport).size_y };
                    let count = capture_pixel_count(width, height)?;
                    let mut pixels = vec![FColor { r: 0, g: 0, b: 0, a: 0 }; count];
                    let device = unsafe { (*viewport).ren_dev.cast() };
                    unsafe { crate::render_device_owner::read_pixels(context.access, device, &mut pixels)?; }
                    let mut file = std::fs::File::create(path).map_err(|_| RuntimeError::new("Unable to create viewport screenshot"))?;
                    write_bmp_pixels(&mut file, width, height, pixels.as_ptr())?;
                    Ok(true)
                }
                crate::camera_exec::ViewportCommand::Snap => {
                    let factor = unsafe { hp2_core::ansi::atoi_checked(cursor.cast()).unwrap_or(0) };
                    let (shrinkage, in_range) = crate::camera_exec::snap_shrinkage(factor);
                    if !in_range { return Err(RuntimeError::new("SNAP shrinkage is outside the source integer range")); }
                    unsafe {
                        (*viewport).snap_u = crate::camera_exec::sdiv((*viewport).size_x, shrinkage);
                        (*viewport).snap_v = crate::camera_exec::sdiv((*viewport).size_y, shrinkage);
                        fill_snap_buffer(context.access, viewport, shrinkage)?;
                    }
                    Ok(true)
                }
                crate::camera_exec::ViewportCommand::Thumbnail => {
                    let thumb_x = unsafe { parse_camera_int(cursor.cast(), &[120, 61, 0]) }?;
                    let thumb_y = unsafe { parse_camera_int(cursor.cast(), &[121, 61, 0]) }?;
                    let (width, height) = crate::camera_exec::thumbnail_dimensions(thumb_x, thumb_y);
                    let shrinkage = unsafe { crate::camera_exec::sdiv((*viewport).size_x, width) };
                    unsafe {
                        (*viewport).snap_u = width;
                        (*viewport).snap_v = height;
                        fill_snap_buffer(context.access, viewport, shrinkage)?;
                    }
                    Ok(true)
                }
                crate::camera_exec::ViewportCommand::SaveThumbnail => {
                    if unsafe { (*viewport).snap_buff.num != 0 } {
                        if let Some(name) = unsafe { parse_camera_token(&mut tail, 64) } {
                            let (width, height, pixels) = unsafe { snap_pixels(viewport)? };
                            let path = save_slot_path(context.access, &name)?;
                            let mut file = std::fs::File::create(path).map_err(|_| RuntimeError::new("Unable to create saved thumbnail"))?;
                            write_bmp_pixels(&mut file, width, height, pixels)?;
                        }
                    }
                    Ok(true)
                }
                crate::camera_exec::ViewportCommand::SaveSnap => {
                    if unsafe { (*viewport).snap_buff.num != 0 } {
                        if let Some(name) = unsafe { parse_camera_token(&mut tail, 64) } {
                            let (source_width, source_height, pixels) = unsafe { snap_pixels(viewport)? };
                            let width = next_power_of_two(source_width)?;
                            let height = next_power_of_two(source_height)?;
                            let path = user_dir_path(&name)?;
                            let mut file = std::fs::File::create(path).map_err(|_| RuntimeError::new("Unable to create saved snap"))?;
                            write_padded_bmp_pixels(&mut file, width, height, source_width, source_height, pixels)?;
                        }
                    }
                    Ok(true)
                }
                crate::camera_exec::ViewportCommand::ShowActors => {
                    require_pawn(actor)?;
                    unsafe { (*actor).show_flags |= crate::camera_exec::SHOW_ACTORS; }
                    Ok(true)
                }
                crate::camera_exec::ViewportCommand::HideActors => {
                    require_pawn(actor)?;
                    unsafe { (*actor).show_flags &= !crate::camera_exec::SHOW_ACTORS; }
                    Ok(true)
                }
                crate::camera_exec::ViewportCommand::RenderMode => {
                    let mode = unsafe { hp2_core::ansi::atoi_checked(cursor.cast()).unwrap_or(0) };
                    if mode > crate::camera_exec::REN_NONE && mode < crate::camera_exec::REN_MAX {
                        require_pawn(actor)?;
                        unsafe { (*actor).rend_map = mode; }
                    }
                    Ok(true)
                }
                crate::camera_exec::ViewportCommand::ExecMacro => {
                    if let Some(filename) = unsafe { parse_camera_token(&mut tail, 64) } {
                        unsafe { exec_macro_core(context, viewport, &filename)?; }
                    }
                    Ok(true)
                }
                crate::camera_exec::ViewportCommand::ShowBones => {
                    require_pawn(actor)?;
                    let on = unsafe { hp2_core::ansi::atoi_checked(cursor.cast()).unwrap_or(0) } != 0;
                    unsafe { crate::camera_exec::set_show_flag(actor, on, crate::camera_exec::SHOW_BONES); }
                    Ok(true)
                }
                crate::camera_exec::ViewportCommand::ShowBounds => {
                    require_pawn(actor)?;
                    let on = unsafe { hp2_core::ansi::atoi_checked(cursor.cast()).unwrap_or(0) } != 0;
                    unsafe { crate::camera_exec::set_show_flag(actor, on, crate::camera_exec::SHOW_BOUNDS); }
                    Ok(true)
                }
                }
            },
        )
    }
}

unsafe fn send_control_text(
    access: CoreAccess<'_>,
    connection: *mut UNetConnection,
    text: *const u32,
) -> RuntimeResult<()> {
    let channel = unsafe { (*connection).channels[0].cast::<UControlChannel>() };
    if channel.is_null()
        || unsafe { (*channel.cast::<hp2_abi::engine::generated::UChannel>()).closing != 0 }
    {
        return Ok(());
    }
    unsafe { crate::channel_owner::serialize_control(access, channel, text) }
}
/// Source `FOutputDevice::Logf(Cmd)` with zero variadic arguments: only literal
/// text and escaped `%%` are defined; numeric/string conversions without their
/// required arguments are reported rather than consuming fabricated stack data.
fn format_logf_without_arguments(format: *const u32) -> RuntimeResult<Vec<u32>> {
    if format.is_null() { return Err(RuntimeError::new("Null UNetConnection Logf format")); }
    let mut output = Vec::with_capacity(128);
    let mut index = 0usize;
    while unsafe { *format.add(index) } != 0 {
        let unit = unsafe { *format.add(index) };
        if unit != b'%' as u32 {
            output.push(unit);
            index += 1;
        } else {
            let mut parsed = index + 1;
            let mut segment = [0 as hp2_core::ansi::Tchar; 512];
            segment[0] = b'%' as hp2_core::ansi::Tchar;
            let mut segment_len = 1usize;
            let mut length = hp2_core::printf_format::Length::None;
            let mut conversion = 0 as hp2_core::ansi::Tchar;
            let action = unsafe {
                hp2_core::printf_format::parse_directive(
                    format.cast(),
                    &mut parsed,
                    segment.as_mut_ptr(),
                    segment.len(),
                    &mut segment_len,
                    0,
                    0,
                    &mut length,
                    &mut conversion,
                )
            };
            if action != hp2_core::printf_format::DirectiveAction::Done
                || conversion != b'%' as hp2_core::ansi::Tchar
            {
                return Err(RuntimeError::new("UNetConnection::Logf format requires unavailable arguments"));
            }
            output.push(b'%' as u32);
            index = parsed;
        }
        if output.len() >= 1023 {
            return Err(RuntimeError::new("UNetConnection::Logf output exceeds appVswprintfCompat capacity"));
        }
    }
    output.push(0);
    Ok(output)
}
fn match_command(
    cursor: &mut *const crate::params::TChar,
    token: *const crate::params::TChar,
) -> bool {
    let stream = (cursor as *mut *const crate::params::TChar).cast::<*const hp2_core::ansi::Tchar>();
    unsafe { hp2_core::command::parse_command(stream, token.cast()) != 0 }
}
unsafe fn parse_camera_int(
    command: *const u32,
    key: &[hp2_core::ansi::Tchar],
) -> RuntimeResult<Option<i32>> {
    let mut text = [0 as hp2_core::ansi::Tchar; 256];
    if !unsafe { hp2_core::text_parse::parse_string(command.cast(), key.as_ptr(), text.as_mut_ptr(), text.len() as i32) } {
        return Ok(None);
    }
    unsafe { hp2_core::ansi::atoi_checked(text.as_ptr().cast()) }
        .map(Some)
        .map_err(|value| {
            RuntimeError::new("THUMBNAIL argument exceeds the INT range")
                .with_context(&value.to_string())
        })
}

unsafe fn fill_snap_buffer(
    access: CoreAccess<'_>,
    viewport: *mut UViewport,
    shrinkage: i32,
) -> RuntimeResult<()> {
    let (width, height, snap_width, snap_height, device) = unsafe {
        ((*viewport).size_x, (*viewport).size_y, (*viewport).snap_u, (*viewport).snap_v, (*viewport).ren_dev)
    };
    if width < 0 || height < 0 || (width > 0 && height > i32::MAX / width) {
        return Err(RuntimeError::new("Invalid viewport capture dimensions"));
    }
    if snap_width < 0 || snap_height < 0 || (snap_width > 0 && snap_height > i32::MAX / snap_width) {
        return Err(RuntimeError::new("Invalid viewport snap dimensions"));
    }
    if shrinkage < 0 { return Err(RuntimeError::new("Negative viewport snap shrinkage")); }
    let source_count = width.checked_mul(height).ok_or_else(|| RuntimeError::new("Viewport capture size overflow"))?;
    let destination_count = snap_width.checked_mul(snap_height).ok_or_else(|| RuntimeError::new("Viewport snap size overflow"))?;
    let mut source = vec![FColor { r: 0, g: 0, b: 0, a: 0 }; source_count as usize];
    unsafe { crate::render_device_owner::read_pixels(access, device.cast(), &mut source)?; }

    let width_bytes = std::mem::size_of::<FColor>() as i32;
    unsafe {
        crate::camera_exec::fill_snap_buffer_with(
            viewport,
            source.as_ptr(),
            width,
            shrinkage,
            destination_count,
            |viewport| {
                let raw = std::ptr::addr_of_mut!((*viewport).snap_buff);
                let slots = hp2_core::array_storage::Hp2ArraySlots {
                    data: std::ptr::addr_of_mut!((*raw).data),
                    num: std::ptr::addr_of_mut!((*raw).num),
                    max: std::ptr::addr_of_mut!((*raw).max),
                };
                hp2_core::array_storage::empty(slots, 0, width_bytes, std::ptr::null(), hp2_core::array_storage::system_allocator())
                    .map_err(|_| RuntimeError::new("Unable to empty canonical viewport snap buffer"))
            },
            |viewport, count| {
                let raw = std::ptr::addr_of_mut!((*viewport).snap_buff);
                let slots = hp2_core::array_storage::Hp2ArraySlots {
                    data: std::ptr::addr_of_mut!((*raw).data),
                    num: std::ptr::addr_of_mut!((*raw).num),
                    max: std::ptr::addr_of_mut!((*raw).max),
                };
                hp2_core::array_storage::add(slots, count, width_bytes, std::ptr::null(), hp2_core::array_storage::system_allocator())
                    .map(|_| ())
                    .map_err(|_| RuntimeError::new("Unable to grow canonical viewport snap buffer"))
            },
            |pixel| {
                let pixel = unsafe { &*pixel };
                Ok::<[i32; 4], RuntimeError>([i32::from(pixel.r), i32::from(pixel.g), i32::from(pixel.b), i32::from(pixel.a)])
            },
        )?;
    }
    Ok(())
}
unsafe fn parse_camera_token(cursor: &mut *const u32, capacity: usize) -> Option<String> {
    if capacity == 0 || capacity > i32::MAX as usize { return None; }
    let mut units = vec![0 as hp2_core::ansi::Tchar; capacity];
    if !unsafe { hp2_core::tokenize::parse_token((cursor as *mut *const u32).cast(), units.as_mut_ptr(), capacity as i32, false) } {
        return None;
    }
    let mut text = String::with_capacity(capacity.min(256));
    for unit in units {
        if unit == 0 { break; }
        let Some(character) = char::from_u32(unit as u32) else { return None; };
        text.push(character);
    }
    Some(text)
}
unsafe fn exec_macro_core(
    context: &mut ExecContext<'_, '_, '_>,
    viewport: *mut UViewport,
    filename: &str,
) -> RuntimeResult<()> {
    let access = context.access;
    let output = std::cell::RefCell::new(&mut *context.output);
    let class = access.native_class("TextBuffer")?;
    let mut filename_units = filename.chars().map(|character| character as crate::params::TChar).collect::<Vec<_>>();
    filename_units.push(0);
    unsafe {
        crate::camera_exec::exec_macro_with(
            filename_units.as_ptr(),
            |_file| {
                let raw_path = std::path::PathBuf::from(filename);
                let path = if raw_path.is_absolute() { raw_path } else { access.data_root().join(raw_path) };
                let bytes = match std::fs::read(path) {
                    Ok(bytes) => bytes,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                    Err(_) => return Err(RuntimeError::new("Unable to import viewport Exec macro text")),
                };
                let mut units = hp2_core::file_persistence::decode_text_file(&bytes)
                    .ok_or_else(|| RuntimeError::new("Unable to decode viewport Exec macro text"))?
                    .into_iter().map(|unit| unit as u32).collect::<Vec<_>>();
                units.push(0);
                let object = unsafe {
                    access.construct_object(class, access.transient_package(), 0, 0, std::ptr::null_mut())?
                };
                if let Err(error) = unsafe {
                    access.assign_fstring(
                        std::ptr::addr_of_mut!((*object.cast::<hp2_abi::engine::generated::UTextBuffer>()).text),
                        &units,
                    )
                } {
                    unsafe { access.conditional_destroy(object)?; }
                    return Err(error);
                }
                Ok(Some(object.cast::<hp2_abi::engine::generated::UTextBuffer>()))
            },
            |file| {
                let name = unsafe { tchar_string(file) };
                (output.borrow_mut())(&format!("FileNotFound: {name}").chars().map(u32::from).collect::<Vec<_>>())
            },
            |text| { unsafe { access.add_root(text.cast()); } Ok(()) },
            |file| {
                let message = format!("Execing {}", unsafe { tchar_string(file) });
                let category = access.intern_str("Log", hp2_core::name_owner::FindName::Add)?;
                access.emit_log(category, &message.chars().map(u32::from).collect::<Vec<_>>())
            },
            |text| {
                let data = unsafe { (**text).text.data };
                if data.is_null() { return Err(RuntimeError::new("Viewport macro text has no storage")); }
                Ok(data.cast())
            },
            |cursor, line| {
                let stream = (cursor as *mut *const crate::params::TChar)
                    .cast::<*const hp2_core::ansi::Tchar>();
                Ok(unsafe {
                    hp2_core::tokenize::parse_line(
                        stream,
                        line.as_mut_ptr().cast(),
                        line.len() as i32,
                        false,
                    )
                })
            },
            |line| unsafe { exec(access, viewport, line.cast(), &mut **output.borrow_mut()).map(|_| ()) },
            |text| {
                access.remove_root(text.cast());
                unsafe { access.conditional_destroy(text.cast()) }.map(|_| ())
            },
        )
    }
}

unsafe fn tchar_string(text: *const crate::params::TChar) -> String {
    if text.is_null() { return String::new(); }
    let mut result = String::new();
    let mut cursor = text;
    while unsafe { *cursor } != 0 {
        result.push(char::from_u32(unsafe { *cursor } as u32).unwrap_or(char::REPLACEMENT_CHARACTER));
        cursor = unsafe { cursor.add(1) };
    }
    result
}

fn capture_pixel_count(width: i32, height: i32) -> RuntimeResult<usize> {
    if width < 0 || height < 0 || (width > 0 && height > i32::MAX / width) {
        return Err(RuntimeError::new("Invalid viewport capture dimensions"));
    }
    usize::try_from(width.checked_mul(height).ok_or_else(|| RuntimeError::new("Viewport capture size overflow"))?)
        .map_err(|_| RuntimeError::new("Viewport capture size is invalid"))
}

fn user_dir_path(filename: &str) -> RuntimeResult<std::path::PathBuf> {
    let directory = hp2_core::unix_identity::user_dir();
    if directory.is_null() { return Err(RuntimeError::new("Core appUserDir storage is unavailable")); }
    let mut text = String::new();
    let mut unit = directory;
    while unsafe { *unit } != 0 {
        let scalar = unsafe { *unit } as u32;
        let character = char::from_u32(scalar)
            .ok_or_else(|| RuntimeError::new("Core appUserDir contains an invalid TCHAR scalar"))?;
        text.push(character);
        unit = unsafe { unit.add(1) };
    }
    text.push_str(filename);
    Ok(std::path::PathBuf::from(text))
}

fn unique_shot_path() -> RuntimeResult<Option<std::path::PathBuf>> {
    let index = crate::camera_exec::unique_shot_index_with(|index| {
        Ok::<bool, RuntimeError>(!user_dir_path(&format!("Shot{index:04}.bmp"))?.exists())
    })?;
    index.map(|index| user_dir_path(&format!("Shot{index:04}.bmp"))).transpose()
}

unsafe fn snap_pixels(viewport: *mut UViewport) -> RuntimeResult<(i32, i32, *const FColor)> {
    let snap = unsafe { (*viewport).snap_buff };
    let count = unsafe { (*viewport).snap_u.checked_mul((*viewport).snap_v) }
        .ok_or_else(|| RuntimeError::new("Viewport snap size overflow"))?;
    if snap.num < count || snap.num < 0 || (snap.num > 0 && snap.data.is_null()) {
        return Err(RuntimeError::new("Viewport snap buffer is not populated"));
    }
    Ok((unsafe { (*viewport).snap_u }, unsafe { (*viewport).snap_v }, snap.data.cast()))
}

fn save_slot_path(access: CoreAccess<'_>, filename: &str) -> RuntimeResult<std::path::PathBuf> {
    let system = access.system()?;
    let path = unsafe { std::ptr::addr_of!((*system).save_slot_path).read_unaligned() };
    if path.num < 0 || (path.num > 0 && path.data.is_null()) {
        return Err(RuntimeError::new("Invalid canonical SaveSlotPath"));
    }
    let prefix: String = (0..path.num.max(0) as usize).map(|index| {
        let unit = unsafe { path.data.cast::<u32>().add(index).read_unaligned() };
        char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER)
    }).take_while(|character| *character != '\0').collect();
    Ok(std::path::PathBuf::from(prefix).join(filename))
}

fn write_bmp_pixels(
    file: &mut std::fs::File,
    width: i32,
    height: i32,
    pixels: *const FColor,
) -> RuntimeResult<()> {
    let count = capture_pixel_count(width, height)?;
    if count > 0 && pixels.is_null() { return Err(RuntimeError::new("Null viewport capture pixels")); }
    crate::camera_exec::write_bitmap_headers_with(
        u32::try_from(width).map_err(|_| RuntimeError::new("Invalid BMP width"))?,
        u32::try_from(height).map_err(|_| RuntimeError::new("Invalid BMP height"))?,
        |bytes| std::io::Write::write_all(file, bytes).map_err(|_| RuntimeError::new("Unable to write viewport BMP")),
        |_| RuntimeError::new("Viewport BMP exceeds the source file format range"),
    )?;
    for y in (0..height).rev() {
        for x in 0..width {
            let pixel = unsafe { pixels.add((y * width + x) as usize) };
            unsafe {
                crate::camera_exec::write_pixel_with(pixel, |bytes| {
                    std::io::Write::write_all(file, bytes).map_err(|_| RuntimeError::new("Unable to write viewport BMP pixel"))
                })?;
            }
        }
    }
    Ok(())
}

fn write_padded_bmp_pixels(
    file: &mut std::fs::File,
    width: i32,
    height: i32,
    source_width: i32,
    source_height: i32,
    pixels: *const FColor,
) -> RuntimeResult<()> {
    let count = capture_pixel_count(source_width, source_height)?;
    if count > 0 && pixels.is_null() { return Err(RuntimeError::new("Null viewport snap pixels")); }
    crate::camera_exec::write_bitmap_headers_with(
        u32::try_from(width).map_err(|_| RuntimeError::new("Invalid BMP width"))?,
        u32::try_from(height).map_err(|_| RuntimeError::new("Invalid BMP height"))?,
        |bytes| std::io::Write::write_all(file, bytes).map_err(|_| RuntimeError::new("Unable to write viewport snap BMP")),
        |_| RuntimeError::new("Viewport snap BMP exceeds the source file format range"),
    )?;
    let first = pixels;
    for y in (0..height).rev() {
        if y >= source_height {
            for _ in 0..width { unsafe { crate::camera_exec::write_pixel_with(first, |bytes| std::io::Write::write_all(file, bytes).map_err(|_| RuntimeError::new("Unable to write viewport snap padding")))?; } }
            continue;
        }
        for x in 0..width {
            let pixel = if x < source_width {
                unsafe { pixels.add((y * source_width + x) as usize) }
            } else { first };
            unsafe { crate::camera_exec::write_pixel_with(pixel, |bytes| std::io::Write::write_all(file, bytes).map_err(|_| RuntimeError::new("Unable to write viewport snap pixel")))?; }
        }
    }
    Ok(())
}

fn next_power_of_two(value: i32) -> RuntimeResult<i32> {
    match crate::camera_exec::shared_power_of_two_size(value) {
        Some((size, factor)) if factor < 31 => Ok(size),
        _ => Err(RuntimeError::new("Viewport snap exceeds the source power-of-two range")),
    }
}
fn report_viewport(
    context: &mut ExecContext<'_, '_, '_>,
    viewport: *mut UViewport,
    command: *const u32,
) -> RuntimeResult<()> {
    let actor = unsafe { (*viewport).actor.cast::<APlayerPawn>() };
    require_pawn(actor)?;
    let level = unsafe { (*actor).base.base.x_level.cast::<ULevel>() };
    if level.is_null() { return Err(RuntimeError::new("REPORT player pawn has no level")); }
    let actor_class = unsafe {
        (*actor.cast::<hp2_abi::engine::generated::UObject>())
            .class.cast::<hp2_abi::engine::generated::UClass>()
    };
    if actor_class.is_null() { return Err(RuntimeError::new("REPORT player pawn has no class")); }
    let class_name = context.access.name_text(i32::from_ne_bytes(unsafe {
        (*actor_class.cast::<hp2_abi::engine::generated::UObject>()).name
    }))?;
    let defaults = crate::url_owner::UrlOwner::current_defaults(context.access)?;
    let mut raw_url = unsafe {
        crate::url_owner::UrlOwner::string(
            context.access,
            &defaults,
            std::ptr::addr_of!((*level).base.url),
            false,
        )?
    };
    if raw_url.num < 0 || (raw_url.num > 0 && raw_url.data.is_null()) {
        return Err(RuntimeError::new("REPORT URL string has invalid storage"));
    }
    let url = if raw_url.num == 0 {
        String::new()
    } else {
        unsafe { std::slice::from_raw_parts(raw_url.data.cast::<u32>(), raw_url.num as usize) }
            .iter().copied().take_while(|unit| *unit != 0)
            .map(|unit| char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER)).collect::<String>()
    };
    unsafe { context.access.destroy_owned_fstring(&mut raw_url as *mut _) ?; }
    let location = unsafe { (*actor).base.base.location };
    let command = unsafe { std::slice::from_raw_parts(command, hp2_core::native_text::text_len(command)) };
    let command = command.iter().copied()
        .map(|unit| char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER)).collect::<String>();
    let class_name = class_name.iter().copied().take_while(|unit| *unit != 0)
        .map(|unit| char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER)).collect::<String>();
    let game = find_game_info(context.access, level)?;
    let game_line = if game.is_null() {
        "   Network client\r\n".to_owned()
    } else {
        let game_class = unsafe { (*game.cast::<hp2_abi::engine::generated::UObject>()).class.cast::<hp2_abi::engine::generated::UClass>() };
        if game_class.is_null() { return Err(RuntimeError::new("REPORT game actor has no class")); }
        let game_name = context.access.name_text(i32::from_ne_bytes(unsafe {
            (*game_class.cast::<hp2_abi::engine::generated::UObject>()).name
        }))?;
        let game_name = game_name.iter().copied().take_while(|unit| *unit != 0)
            .map(|unit| char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER)).collect::<String>();
        let field = context.access.intern_str("Difficulty", hp2_core::name_owner::FindName::Find)?;
        let property_class = context.access.native_class("ByteProperty")?;
        let property = unsafe { context.access.find_field(game_class.cast(), field, property_class.cast()) }
            .cast::<hp2_abi::engine::generated::UProperty>();
        if property.is_null() { return Err(RuntimeError::new("REPORT game has no Difficulty property")); }
        let offset = unsafe { (*property).offset };
        if offset < 0 { return Err(RuntimeError::new("REPORT game Difficulty offset is invalid")); }
        let difficulty = unsafe { game.cast::<u8>().add(offset as usize).read() };
        format!("   Game class: {game_name}\r\n   Difficulty: {difficulty}\r\n")
    };
    let report = format!(
        "Report: {command}\r\n   Version: {}\r\n   Player class: {class_name}\r\n   URL: {url}\r\n   Location: {} {} {}\r\n{game_line}",
        env!("CARGO_PKG_VERSION"),
        location.x.round() as i32,
        location.y.round() as i32,
        location.z.round() as i32,
    );
    let units = report.chars().map(u32::from).collect::<Vec<_>>();
    (context.output)(&units)?;
    #[cfg(feature = "sdl")]
    if !hp2_platform::clipboard::set_clipboard_text(&report) {
        return Err(RuntimeError::new("Unable to copy viewport report to the clipboard"));
    }
    #[cfg(not(feature = "sdl"))]
    return Err(RuntimeError::new("Viewport report clipboard is unavailable without the SDL feature"));
    #[cfg(feature = "sdl")]
    Ok(())
}

fn require_pawn(actor: *mut APlayerPawn) -> RuntimeResult<()> {
    if actor.is_null() { Err(RuntimeError::new("Viewport camera command has no player pawn")) } else { Ok(()) }
}
fn find_game_info(access: CoreAccess<'_>, level: *mut ULevel) -> RuntimeResult<*mut c_void> {
    let level_info_class = access.native_class("Engine.LevelInfo")?;
    let actors = unsafe { (*level).base.actors };
    if actors.array_num < 0 || actors.array_num > actors.array_max
        || (actors.array_num > 0 && actors.data.is_null())
    {
        return Err(RuntimeError::new("REPORT level actor array is invalid"));
    }
    for index in 0..actors.array_num as usize {
        let actor = unsafe { actors.data.cast::<*mut AActor>().add(index).read_unaligned() };
        if actor.is_null() { continue; }
        let class = unsafe { (*actor.cast::<hp2_abi::engine::generated::UObject>()).class };
        if unsafe { hp2_core::object_model::is_child_of(class, level_info_class.cast()) } {
            return Ok(unsafe { (*actor.cast::<ALevelInfo>()).game.cast() });
        }
    }
    Ok(std::ptr::null_mut())
}

/// Resets the canonical viewport's input using `UInput::ResetInput`.
///
/// # Safety
/// `viewport` must be a live canonical `UViewport` owned by `access`.
pub unsafe fn reset_input(access: CoreAccess<'_>, viewport: *mut UViewport) -> RuntimeResult<()> {
    if viewport.is_null() {
        return Err(RuntimeError::new("Null canonical UViewport in input reset"));
    }
    let input = unsafe { (*viewport).input.cast::<hp2_abi::engine::generated::UInput>() };
    if input.is_null() {
        return Err(RuntimeError::new("Canonical UViewport has no UInput"));
    }
    unsafe { crate::input_owner::reset_input(access, input) }
}

#[cfg(test)]
mod render_host_contracts {
    use super::*;
    use std::ptr;

    #[test]
    fn viewport_exec_uses_its_own_player_even_without_actor_player_backlink() {
        let owner = crate::render_owner::render_test_support::owner("viewport-exec-receiver");
        let access = owner.access();
        unsafe {
            let actor = crate::render_owner::render_test_support::construct(access, "Engine.Camera")
                .cast::<APlayerPawn>();
            let level = crate::render_owner::render_test_support::construct(access, "Engine.Level")
                .cast::<ULevel>();
            let info = crate::render_owner::render_test_support::construct(access, "Engine.LevelInfo")
                .cast::<ALevelInfo>();
            (*info).set_b_begun_play(true);
            (*actor.cast::<AActor>()).level = info.cast();
            (*actor.cast::<AActor>()).x_level = level.cast();
            (*actor).player = ptr::null_mut();
            let mut viewport: UViewport = std::mem::zeroed();
            viewport.actor = actor.cast();
            let class = (*actor.cast::<hp2_abi::engine::generated::UObject>()).class
                .cast::<hp2_abi::engine::generated::UStruct>();
            let property_name = access.intern_str("DrawScale", hp2_core::name_owner::FindName::Find).unwrap();
            let property = access.find_field(class, property_name, access.native_class("Core.Property").unwrap());
            assert!(!property.is_null());
            let function = access.construct_object(
                access.native_class("Core.Function").unwrap(), class.cast(),
                access.intern_str("SetViewportScale", hp2_core::name_owner::FindName::Add).unwrap(),
                0, ptr::null_mut(),
            ).unwrap().cast::<hp2_abi::engine::generated::UFunction>();
            (*function).function_flags = 0x200;
            (*function).return_value_offset = u16::MAX;
            (*function.cast::<hp2_abi::engine::generated::UStruct>()).properties_align = 4;
            let state = class.cast::<hp2_abi::engine::generated::UState>();
            let hash = i32::from_ne_bytes((*function.cast::<hp2_abi::engine::generated::UObject>()).name) as usize & 255;
            (*function.cast::<hp2_abi::engine::generated::UField>()).hash_next = (*state).vf_hash[hash];
            (*state).vf_hash[hash] = function.cast();
            let mut code = vec![0x0f, 0x01];
            code.extend((*property.cast::<hp2_abi::engine::generated::UObject>()).index.to_le_bytes());
            code.push(0x1e);
            code.extend(3.0_f32.to_le_bytes());
            code.extend([0x04, 0x0b]);
            let array = &raw mut (*function.cast::<hp2_abi::engine::generated::UStruct>()).script;
            hp2_core::array_storage::set(hp2_core::array_storage::Hp2ArraySlots {
                data: &raw mut (*array).data, num: &raw mut (*array).num, max: &raw mut (*array).max,
            }, code.len() as i32, 1, ptr::null(), hp2_core::array_storage::system_allocator()).unwrap();
            ptr::copy_nonoverlapping(code.as_ptr(), (*array).data.cast(), code.len());
            (*actor.cast::<AActor>()).draw_scale = 1.0;
            let command: Vec<u32> = "SetViewportScale".chars().map(u32::from).chain([0]).collect();
            let mut output = |_text: &[u32]| Ok(());
            assert!(exec(access, &raw mut viewport, command.as_ptr(), &mut output).unwrap());
            assert_eq!((*actor.cast::<AActor>()).draw_scale, 3.0);
            assert!((*actor).player.is_null(), "command dispatch does not manufacture a backlink");
        }
    }

    #[test]
    fn package_loaded_viewport_actor_executes_inherited_console_command() {
        let owner = crate::render_owner::render_test_support::owner("inherited-viewport-exec");
        let access = owner.access();
        unsafe {
            let actor = crate::render_owner::render_test_support::construct(access, "Engine.Camera")
                .cast::<APlayerPawn>();
            let info = crate::render_owner::render_test_support::construct(access, "Engine.LevelInfo")
                .cast::<ALevelInfo>();
            (*info).set_b_begun_play(true);
            (*actor.cast::<AActor>()).level = info.cast();
            let level = crate::render_owner::render_test_support::construct(access, "Engine.Level")
                .cast::<ULevel>();
            (*actor.cast::<AActor>()).x_level = level.cast();
            (*actor.cast::<hp2_abi::engine::generated::APawn>()).set_b_behind_view(false);
            let command: Vec<u32> = "BehindView True".chars().map(u32::from).chain([0]).collect();
            let mut output = |_text: &[u32]| Ok(());
            assert!(crate::engine_exec_owner::process_console_exec(
                access, actor.cast(), actor.cast(), command.as_ptr(), &mut output,
            ).unwrap(), "package-loaded inherited exec must resolve without manually populated hashes");
            assert!((*actor.cast::<hp2_abi::engine::generated::APawn>()).b_behind_view());
        }
    }
}
