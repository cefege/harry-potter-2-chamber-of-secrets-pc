//! Core-owned adapter for `UInput` behavior.
use hp2_abi::engine::generated::{AActor, FAlias, FName, TArrayRaw, UEnum, UField, UInput, UObject, UProperty, UViewport, UStruct};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::name_owner::FindName;
use std::{ffi::c_void, mem, ptr};

const KIND_BYTE: i32 = 1;
const KIND_FLOAT: i32 = 2;
const CPF_CONFIG: u32 = 0x0000_4000;
const CPF_INPUT: u32 = 0x0000_0004;
const RF_PUBLIC: u32 = 4;
const ALIAS_MAX: usize = 40;

/// Reproduces `UInput::StaticInitInput` using live canonical EInputKey metadata.
///
/// # Safety
/// Core's class graph and compiled `AActor.EInputKey` must be initialized,
/// matching the GameEngine InitInputSystem phase rather than class bootstrap.
pub(crate) unsafe fn static_init_input(access: CoreAccess<'_>) -> RuntimeResult<()> {
    let class = access.native_class("Engine.Input")?;
    let alias_name = access.intern_str("Alias", FindName::Add)?;
    let alias_struct = unsafe {
        access.create_cpp_struct(
            class.cast(),
            alias_name,
            mem::size_of::<FAlias>(),
            RF_PUBLIC,
        )?
    };
    let empty_category = access.intern_str("", FindName::Find)?;
    let alias_field = access.intern_str("Alias", FindName::Add)?;
    let command_field = access.intern_str("Command", FindName::Add)?;
    unsafe {
        access.add_cpp_property(
            alias_struct, "NameProperty", alias_field, empty_category,
            mem::offset_of!(FAlias, alias), 1, CPF_CONFIG, RF_PUBLIC, ptr::null_mut(),
        )?;
        access.add_cpp_property(
            alias_struct, "StrProperty", command_field, empty_category,
            mem::offset_of!(FAlias, command), 1, CPF_CONFIG, RF_PUBLIC, ptr::null_mut(),
        )?;
        access.link_cpp_struct(alias_struct)?;
    }
    let aliases_name = access.intern_str("Aliases", FindName::Add)?;
    let aliases_category = access.intern_str("Aliases", FindName::Add)?;
    unsafe {
        access.add_cpp_property(
            class.cast(), "StructProperty", aliases_name, aliases_category,
            mem::offset_of!(UInput, aliases), ALIAS_MAX, CPF_CONFIG, RF_PUBLIC, alias_struct,
        )?;
    }

    let actor_class = access.native_class("Engine.Actor")?;
    let enum_class = access.native_class("Core.Enum")?;
    let enum_name = access.intern_str("EInputKey", FindName::Find)?;
    let input_keys = unsafe { access.find_object(enum_class, actor_class.cast(), enum_name, false) }
        .cast::<UEnum>();
    if input_keys.is_null() { return Err(RuntimeError::new("Compiled AActor.EInputKey metadata is unavailable")); }
    let names = unsafe { ptr::addr_of!((*input_keys).names).read_unaligned() };
    if names.num != 255 || names.max < names.num || names.data.is_null() {
        return Err(RuntimeError::new("Compiled EInputKey names do not match IK_MAX"));
    }
    let raw_keys = access.intern_str("RawKeys", FindName::Add)?;
    for index in 0..255usize {
        let key = unsafe { names.data.cast::<FName>().add(index).read_unaligned() };
        if key.index == 0 { continue; }
        let text = access.name_text(key.index)?;
        // StaticInitInput uses Str + 3, including source keys spelled UK_*.
        let suffix = text.get(3..)
            .ok_or_else(|| RuntimeError::new("Compiled EInputKey name is shorter than its three-character prefix"))?;
        let property_name = access.intern(suffix, FindName::Add)?;
        let offset = mem::offset_of!(UInput, bindings)
            .checked_add(index.checked_mul(mem::size_of::<TArrayRaw>())
                .ok_or_else(|| RuntimeError::new("UInput binding offset overflow"))?)
            .ok_or_else(|| RuntimeError::new("UInput binding offset overflow"))?;
        unsafe {
            access.add_cpp_property(
                class.cast(), "StrProperty", property_name, raw_keys, offset, 1,
                CPF_CONFIG, RF_PUBLIC, ptr::null_mut(),
            )?;
        }
    }
    unsafe {
        access.link_cpp_struct(class.cast())?;
        let defaults = access.default_object(class)?;
        access.load_config(defaults)?;
    }
    Ok(())
}

/// Initializes one canonical input for its viewport, then applies the actual
/// `ResetInput`/`UpdateInput(1)` sequence and source Init log.
///
/// # Safety
/// `input` and `viewport` must be live canonical Engine objects.
pub(crate) unsafe fn init(
    access: CoreAccess<'_>,
    input: *mut UInput,
    viewport: *mut UViewport,
) -> RuntimeResult<()> {
    if input.is_null() || viewport.is_null() {
        return Err(RuntimeError::new("UInput::Init requires live input and viewport objects"));
    }
    unsafe { (*input).viewport = viewport.cast(); }
    unsafe { reset_input(access, input)?; }
    let name_index = i32::from_ne_bytes(unsafe { (*viewport.cast::<UObject>()).name });
    let name = access.name_text(name_index)?;
    let mut message = vec![73, 110, 112, 117, 116, 32, 115, 121, 115, 116, 101, 109, 32, 105, 110, 105, 116, 105, 97, 108, 105, 122, 101, 100, 32, 102, 111, 114, 32];
    message.extend(name);
    let category = access.intern_str("Init", FindName::Add)?;
    access.emit_log(category, &message)
}
/// Dispatches one `UInput::Process` key state through canonical bindings.
///
/// # Safety
/// `input` must be a live canonical input object with a live viewport.
pub(crate) unsafe fn process(
    access: CoreAccess<'_>,
    input: *mut UInput,
    key: i32,
    state: i32,
    delta: f32,
    output: &mut crate::exec_output::ExecOutput<'_>,
) -> RuntimeResult<i32> {
    if input.is_null() { return Err(RuntimeError::new("Null canonical UInput")); }
    if !(0..255).contains(&key) { return Err(RuntimeError::new("UInput key is outside IK_MAX")); }
    let viewport = unsafe { (*input).viewport };
    if viewport.is_null() { return Err(RuntimeError::new("UInput has no viewport")); }
    let binding = unsafe { ptr::addr_of!((*input).bindings[key as usize]).read_unaligned() };
    if binding.num < 0 || (binding.num > 0 && binding.data.is_null()) {
        return Err(RuntimeError::new("UInput binding has invalid FString storage"));
    }
    let has_binding = binding.num > 1;
    let output = std::cell::RefCell::new(output);
    unsafe {
        crate::input::process_binding_with(input, state, delta, has_binding, || {
            let command = binding.data.cast::<u32>();
            crate::input::exec_input_commands_with(
                input,
                command,
                |cursor, line, max| {
                    // SAFETY: command buffers are NUL-terminated TCHAR arrays and have
                    // the source fixed 256-unit output capacity.
                    Ok(unsafe {
                        hp2_core::tokenize::parse_line(cursor.cast(), line.cast(), max, false)
                    })
                },
                |cursor, token| {
                    // SAFETY: both command pointers are NUL-terminated TCHAR arrays.
                    Ok(unsafe { hp2_core::command::parse_command(cursor.cast(), token.cast()) != 0 })
                },
                |viewport, command| unsafe {
                    crate::camera_owner::exec(
                        access,
                        viewport.cast::<UViewport>(),
                        command,
                        &mut **output.borrow_mut(),
                    ).map(|_| ())
                },
                |input, command| unsafe { exec(access, input, command, &mut **output.borrow_mut()).map(|_| ()) },
            )
        })
    }
}


const fn ascii_token<const N: usize>(text: &[u8]) -> [hp2_core::ansi::Tchar; N] {
    assert!(text.len() == N);
    let mut result = [0; N];
    let mut index = 0;
    while index < N {
        result[index] = text[index] as hp2_core::ansi::Tchar;
        index += 1;
    }
    result
}

fn eq_ascii_case(units: &[u32], ascii: &[u8]) -> bool {
    units.len() == ascii.len() && units.iter().zip(ascii).all(|(&a, &b)| {
        a <= 127 && (a as u8).eq_ignore_ascii_case(&b)
    })
}

unsafe fn find_key(access: CoreAccess<'_>, input: *mut UInput, wanted: &[u32]) -> RuntimeResult<Option<(usize, Vec<u32>)>> {
    let enum_object = unsafe { (*input).input_keys.cast::<UEnum>() };
    if enum_object.is_null() { return Err(RuntimeError::new("UInput key enum metadata is unavailable")); }
    let names = unsafe { (*enum_object).names };
    if names.num < 0 || names.max < names.num || (names.num > 0 && names.data.is_null()) {
        return Err(RuntimeError::new("UInput key enum names have invalid storage"));
    }
    for index in 0..names.num.min(255) as usize {
        let name = unsafe { names.data.cast::<FName>().add(index).read_unaligned() };
        if name.index == 0 { continue; }
        let text = access.name_text(name.index)?;
        let Some(suffix) = text.get(3..) else { continue; };
        if suffix.len() == wanted.len() && suffix.iter().zip(wanted).all(|(&a, &b)| {
            if a <= 127 && b <= 127 { (a as u8).eq_ignore_ascii_case(&(b as u8)) } else { a == b }
        }) {
            return Ok(Some((index, suffix.to_vec())));
        }
    }
    Ok(None)
}

/// Executes a canonical `UInput::Exec` command using Core parsers and metadata.
///
/// # Safety
/// `input` must be live and its viewport and any actor properties accessed must remain live.
pub(crate) unsafe fn exec(
    access: CoreAccess<'_>,
    input: *mut UInput,
    mut command: *const u32,
    output: &mut crate::exec_output::ExecOutput<'_>,
) -> RuntimeResult<bool> {
    if input.is_null() { return Err(RuntimeError::new("Null canonical UInput")); }
    let viewport = unsafe { (*input).viewport.cast::<UViewport>() };
    if viewport.is_null() { return Err(RuntimeError::new("UInput has no viewport")); }
    let actor = unsafe { (*viewport).actor };
    let mut token = [0u32; 256];

    macro_rules! matched {
        ($text:literal) => {{
            const TARGET: [hp2_core::ansi::Tchar; concat!($text, "\0").len()] =
                ascii_token(concat!($text, "\0").as_bytes());
            let stream = (&mut command as *mut *const u32).cast();
            unsafe { hp2_core::command::parse_command(stream, TARGET.as_ptr()) != 0 }
        }};
    }

    // The stock input page sends SET Input <key> <complete binding>. Handle
    // only actual EInputKey names here; all other SET commands must continue
    // through the ordinary viewport/engine command chain.
    {
        let mut cursor = command;
        if unsafe { hp2_core::command::parse_command((&mut cursor as *mut *const u32).cast(), hp2_core::lit!("SET").as_ptr().cast()) } != 0 {
            let mut class = [0u32; 256];
            let mut key = [0u32; 256];
            let has_class = unsafe { hp2_core::tokenize::parse_token((&mut cursor as *mut *const u32).cast(), class.as_mut_ptr().cast(), 256, true) };
            if has_class && eq_ascii_case(&trim_nul(&class), b"Input")
                && unsafe { hp2_core::tokenize::parse_token((&mut cursor as *mut *const u32).cast(), key.as_mut_ptr().cast(), 256, true) }
            {
                if let Some((index, canonical)) = unsafe { find_key(access, input, &trim_nul(&key))? } {
                    while unsafe { *cursor } == b' ' as u32 { cursor = unsafe { cursor.add(1) }; }
                    let mut value = Vec::new();
                    while unsafe { *cursor } != 0 {
                        value.push(unsafe { *cursor });
                        cursor = unsafe { cursor.add(1) };
                    }
                    // Allocate first, then publish through the canonical User cache.
                    // On publication failure the live binding stays unchanged.
                    let class = unsafe { (*input.cast::<UObject>()).class };
                    let defaults = unsafe { access.default_object(class.cast())? }.cast::<UInput>();
                    if defaults == input { return Err(RuntimeError::new("Cannot rebind an input class default")); }
                    let mut replacement: TArrayRaw = unsafe { mem::zeroed() };
                    let mut default_replacement: TArrayRaw = unsafe { mem::zeroed() };
                    let mut terminated = value.clone();
                    terminated.push(0);
                    unsafe { access.assign_fstring(&raw mut replacement, &terminated)?; }
                    if let Err(error) = unsafe { access.assign_fstring(&raw mut default_replacement, &terminated) } {
                        unsafe { access.destroy_owned_fstring(&raw mut replacement)?; }
                        return Err(error);
                    }
                    let changes = vec![("Engine.Input".chars().map(u32::from).collect(), canonical, value)];
                    if let Err(error) = access.config_update_user_staged(&changes) {
                        unsafe {
                            access.destroy_owned_fstring(&raw mut replacement)?;
                            access.destroy_owned_fstring(&raw mut default_replacement)?;
                        }
                        return Err(error);
                    }
                    unsafe {
                        mem::swap(&mut (*input).bindings[index], &mut replacement);
                        mem::swap(&mut (*defaults).bindings[index], &mut default_replacement);
                        access.destroy_owned_fstring(&raw mut replacement)?;
                        access.destroy_owned_fstring(&raw mut default_replacement)?;
                    }
                    return Ok(true);
                }
            }
        }
    }

    let binding_mode = if matched!("BUTTON") {
        Some(0)
    } else if matched!("PULSE") {
        Some(1)
    } else if matched!("TOGGLE") {
        Some(2)
    } else {
        None
    };
    if let Some(binding_mode) = binding_mode {
        let action = unsafe { (*input).action };
        let found = !actor.is_null()
            && unsafe {
                hp2_core::tokenize::parse_token(
                    (&mut command as *mut *const u32).cast(),
                    token.as_mut_ptr().cast(),
                    256,
                    false,
                )
            };
        let property = if found {
            unsafe { find_input_property(access, actor, &token, "ByteProperty")? }
        } else { ptr::null_mut() };
        if property.is_null() {
            return emit(output, b"Bad Button command");
        }
        let value = property.cast::<u8>();
        if action == 1 && binding_mode == 2 {
            unsafe { *value ^= 0x80; }
        } else if action == 1 {
            unsafe { *value = 1; }
        } else if action == 3 && binding_mode == 0 && unsafe { *value != 0 } {
            unsafe { *value = 0; }
        }
        return Ok(true);
    }
    if matched!("AXIS") {
        let found = !actor.is_null()
            && unsafe {
                hp2_core::tokenize::parse_token(
                    (&mut command as *mut *const u32).cast(),
                    token.as_mut_ptr().cast(),
                    256,
                    false,
                )
            };
        let property = if found {
            unsafe { find_input_property(access, actor, &token, "FloatProperty")? }
        } else { ptr::null_mut() };
        if property.is_null() { return emit(output, b"Bad Axis command"); }
        let mut speed = 1.0_f32;
        let mut speed_base = 0.0_f32;
        parse_float(command, hp2_core::lit!("SPEED="), &mut speed);
        parse_float(command, hp2_core::lit!("SPEEDBASE="), &mut speed_base);
        let axis = property.cast::<f32>();
        let delta = unsafe { (*input).delta };
        let action = unsafe { (*input).action };
        if speed_base > 0.0 {
            unsafe { *axis = (delta * speed_base).mul_add(speed, *axis); }
        } else if action == 4 {
            unsafe { *axis = (0.01 * delta).mul_add(speed, *axis); }
        } else if action == 2 {
            unsafe { *axis = delta.mul_add(speed, *axis); }
        }
        return Ok(true);
    }
    if matched!("KEYNAME") {
        let key = unsafe { hp2_core::ansi::atoi_checked(command.cast()).unwrap_or(0) };
        let enum_object = unsafe { (*input).input_keys.cast::<UEnum>() };
        if enum_object.is_null() { return Err(RuntimeError::new("UInput key enum metadata is unavailable")); }
        let names = unsafe { (*enum_object).names };
        if key >= 0 && key < names.num && !names.data.is_null() {
            let name = unsafe { names.data.cast::<FName>().add(key as usize).read_unaligned() };
            if name.index != 0 {
                let text = access.name_text(name.index)?;
                return emit_units(output, text.get(3..).unwrap_or(&[]));
            }
        }
        return Ok(true);
    }
    if matched!("KEYBINDING") {
        if !unsafe {
            hp2_core::tokenize::parse_token(
                (&mut command as *mut *const u32).cast(),
                token.as_mut_ptr().cast(),
                256,
                false,
            )
        } {
            return Ok(true);
        }
        if let Some((key, _)) = unsafe { find_key(access, input, &trim_nul(&token))? } {
            let binding = unsafe { &(*input).bindings[key] };
            if binding.num < 0 || (binding.num > 0 && binding.data.is_null()) {
                return Err(RuntimeError::new("UInput key binding has invalid FString storage"));
            }
            if binding.num > 0 {
                return emit_wide(output, binding.data.cast());
            }
        }
        return Ok(true);
    }
    if !crate::input::in_alias()
        && unsafe {
            hp2_core::tokenize::parse_token(
                (&mut command as *mut *const u32).cast(),
                token.as_mut_ptr().cast(),
                256,
                false,
            )
        }
    {
        let alias_name = access.intern(&trim_nul(&token), FindName::Find)?;
        if alias_name != 0 {
            for index in 0..40usize {
                let alias = unsafe { &(*input).aliases[index] };
                if i32::from_ne_bytes(alias.alias) != alias_name { continue; }
                if alias.command.num < 0 || (alias.command.num > 0 && alias.command.data.is_null()) {
                    return Err(RuntimeError::new("UInput alias has invalid command storage"));
                }
                let output = std::cell::RefCell::new(output);
                return crate::input::with_alias_scope(|| unsafe {
                    crate::input::exec_input_commands_with(
                        input,
                        alias.command.data.cast(),
                        |cursor, line, max| Ok(hp2_core::tokenize::parse_line(cursor.cast(), line.cast(), max, false)),
                        |cursor, text| Ok(hp2_core::command::parse_command(cursor.cast(), text.cast()) != 0),
                        |viewport, text| crate::camera_owner::exec(access, viewport.cast(), text, &mut **output.borrow_mut()).map(|_| ()),
                        |input, text| exec(access, input, text, &mut **output.borrow_mut()).map(|_| ()),
                    ).map(|_| true)
                });
            }
        }
    }
    Ok(false)
}

unsafe fn find_input_property(
    access: CoreAccess<'_>,
    actor: *mut c_void,
    name: &[u32],
    class_name: &str,
) -> RuntimeResult<*mut u8> {
    if actor.is_null() { return Ok(ptr::null_mut()); }
    let actor_object = actor.cast::<UObject>();
    let actor_class = unsafe { (*actor_object).class };
    let name = access.intern(&trim_nul(name), FindName::Find)?;
    if name == 0 { return Ok(ptr::null_mut()); }
    let property_class = access.native_class(class_name)?;
    unsafe { with_input_variables(access, actor_class.cast(), |variables| {
        let properties = ptr::addr_of!((*variables).properties).cast::<*mut UProperty>();
        for index in 0..(*variables).count {
            let property = properties.add(index as usize).read();
            if i32::from_ne_bytes((*property.cast::<UObject>()).name) == name
                && hp2_core::object_model::is_a(property.cast(), property_class.cast())
            {
                let offset = (*property).offset;
                if offset < 0 { return Err(RuntimeError::new("Input actor property has a negative offset")); }
                return Ok(actor.cast::<u8>().add(offset as usize));
            }
        }
        Ok(ptr::null_mut())
    }) }
}

fn trim_nul(text: &[u32]) -> &[u32] {
    &text[..text.iter().position(|unit| *unit == 0).unwrap_or(text.len())]
}
fn emit(output: &mut crate::exec_output::ExecOutput<'_>, text: &[u8]) -> RuntimeResult<bool> {
    let mut wide = [0u32; 64];
    if text.len() > wide.len() { return Err(RuntimeError::new("Input Exec log message exceeds its fixed buffer")); }
    for (target, byte) in wide.iter_mut().zip(text.iter()) { *target = u32::from(*byte); }
    output(&wide[..text.len()])?;
    Ok(true)
}

fn emit_units(output: &mut crate::exec_output::ExecOutput<'_>, text: &[u32]) -> RuntimeResult<bool> {
    let end = text.iter().position(|unit| *unit == 0).unwrap_or(text.len());
    output(&text[..end])?;
    Ok(true)
}

fn emit_wide(output: &mut crate::exec_output::ExecOutput<'_>, text: *const u32) -> RuntimeResult<bool> {
    if text.is_null() { return Ok(true); }
    let mut length = 0usize;
    unsafe { while *text.add(length) != 0 { length += 1; } }
    output(unsafe { std::slice::from_raw_parts(text, length) })?;
    Ok(true)
}


fn ascii_upper(unit: u32) -> u32 {
    if (u32::from(b'a')..=u32::from(b'z')).contains(&unit) {
        unit - u32::from(b'a') + u32::from(b'A')
    } else {
        unit
    }
}

fn parse_float(command: *const u32, key: &[u32], value: &mut f32) {
    let length = |_| Ok::<_, std::convert::Infallible>(key.len() - 1);
    let number = |suffix| {
        let ansi = unsafe { hp2_core::ansi::to_ansi_string(suffix) };
        Ok(unsafe { libc::atof(ansi.cast()) } as f32)
    };
    let result = unsafe {
        hp2_core::parse_scalar::parse_float_with(command.cast(), key.as_ptr().cast(), value, length, number)
    };
    match result {
        Ok(_) => {}
        Err(never) => match never {},
    }
}


/// Runs the source `UInput::ResetInput` sequence against canonical Core records.
///
/// # Safety
/// `input` must be a live canonical `UInput`; its viewport and actor, when
/// required by the source implementation, must remain live during the call.
pub unsafe fn reset_input(access: CoreAccess<'_>, input: *mut UInput) -> RuntimeResult<()> {
    if input.is_null() {
        return Err(RuntimeError::new("Null canonical UInput"));
    }
    let viewport = unsafe { (*input).viewport.cast::<UViewport>() };
    if viewport.is_null() {
        return Err(RuntimeError::new("UInput has no viewport"));
    }
    let actor = unsafe { (*viewport).actor };
    if actor.is_null() {
        return Err(RuntimeError::new("UInput viewport has no actor"));
    }
    unsafe {
        crate::input::reset_input_with(
            input,
            |_viewport, actor, kind| reset_input_properties(access, actor, kind),
            |viewport, reset| crate::platform_owner::update_input(access, viewport.cast(), reset != 0),
        )
    }
}

unsafe fn reset_input_properties(
    access: CoreAccess<'_>,
    actor: *mut c_void,
    kind: i32,
) -> RuntimeResult<()> {
    if actor.is_null() {
        return Err(RuntimeError::new("UInput viewport has no actor"));
    }
    let object = actor.cast::<UObject>();
    let class = unsafe { (*object).class.cast::<UStruct>() };
    if class.is_null() {
        return Err(RuntimeError::new("UInput actor has no class"));
    }
    let wanted: &[u32] = match kind {
        KIND_BYTE => &[66, 121, 116, 101, 80, 114, 111, 112, 101, 114, 116, 121],
        KIND_FLOAT => &[70, 108, 111, 97, 116, 80, 114, 111, 112, 101, 114, 116, 121],
        _ => return Err(RuntimeError::new("Invalid UInput property reset kind")),
    };
    let mut structure = class;
    while !structure.is_null() {
        let mut property = unsafe { (*structure).property_link };
        while !property.is_null() {
            let property_object = property.cast::<UObject>();
            let property_class = unsafe { (*property_object).class.cast::<UObject>() };
            if property_class.is_null() {
                return Err(RuntimeError::new("UInput property has no reflected class"));
            }
            let class_name = i32::from_ne_bytes(unsafe { (*property_class).name });
            if access.name_text(class_name)?.as_slice() == wanted {
                let typed_property = property.cast::<UProperty>();
                if unsafe { (*typed_property).property_flags } & CPF_INPUT != 0 {
                    let offset = unsafe { (*typed_property).offset };
                    if offset < 0 {
                        return Err(RuntimeError::new("UInput property has a negative offset"));
                    }
                    let value = unsafe { actor.cast::<u8>().add(offset as usize) };
                    if kind == KIND_BYTE {
                        unsafe { value.write(0) };
                    } else {
                        unsafe { value.cast::<f32>().write(0.0) };
                    }
                }
            }
            property = unsafe { (*property.cast::<UProperty>()).property_link_next };
        }
        structure = unsafe { (*structure.cast::<hp2_abi::engine::generated::UField>()).super_field.cast() };
    }
    Ok(())
}
/// Runs canonical `UPlayer::ReadInput` for Engine viewports and connections.
///
/// # Safety
/// `player` must be a live canonical `UPlayer`; its input and actor records
/// remain live through synchronous input-command reentry.
pub(crate) unsafe fn player_read_input(
    access: CoreAccess<'_>,
    player: *mut std::ffi::c_void,
    delta_seconds: f32,
) -> RuntimeResult<()> {
    if player.is_null() {
        return Err(RuntimeError::new("Null canonical UPlayer in ReadInput"));
    }
    let object = player.cast::<UObject>();
    let viewport_class = access.native_class("Engine.Viewport")?;
    if unsafe { hp2_core::object_model::is_a(object.cast(), viewport_class.cast()) } {
        let viewport = player.cast::<UViewport>();
        if delta_seconds != -1.0 {
            unsafe {
                crate::platform_owner::update_input(
                    access,
                    player.cast::<hp2_abi::engine::generated::USDLViewport>(),
                    false,
                )?;
            }
        }
        let input = unsafe { ptr::addr_of!((*viewport).input).read_unaligned().cast::<UInput>() };
        if input.is_null() {
            return Err(RuntimeError::new("Canonical UViewport has no UInput"));
        }
        return unsafe { read_input(access, input, delta_seconds) };
    }

    let connection_class = access.native_class("Engine.NetConnection")?;
    if unsafe { hp2_core::object_model::is_a(object.cast(), connection_class.cast()) } {
        return Ok(());
    }
    Err(RuntimeError::new("Core UPlayer::ReadInput has no direct implementation for this class"))
}

unsafe fn for_each_input_property(
    class: *mut UStruct,
    property_class: *mut hp2_abi::engine::generated::UClass,
    mut visit: impl FnMut(*mut UProperty),
) {
    let mut structure = class;
    while !structure.is_null() {
        let mut field = unsafe { (*structure).children };
        while !field.is_null() {
            if unsafe { hp2_core::object_model::is_a(field.cast(), property_class.cast()) } {
                let property = field.cast::<UProperty>();
                if unsafe { (*property).property_flags } & CPF_INPUT != 0 {
                    visit(property);
                }
            }
            field = unsafe { (*field.cast::<UField>()).next };
        }
        structure = unsafe { (*structure.cast::<UField>()).super_field.cast() };
    }
}

unsafe fn with_input_variables<T>(
    access: CoreAccess<'_>,
    class: *mut hp2_abi::engine::generated::UClass,
    operation: impl FnOnce(*mut crate::input::InputVarCache) -> RuntimeResult<T>,
) -> RuntimeResult<T> {
    if class.is_null() {
        return Err(RuntimeError::new("Input actor has no reflected class"));
    }
    let state = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Canonical input has no Engine cache owner"))?;
    let id = hp2_core::cache_id::frame(0x1f, unsafe { (*class.cast::<UObject>()).index } as u32);
    let cached = {
        let mut cache = state.cache.try_borrow_mut()
            .map_err(|_| RuntimeError::new("Canonical input cache is already borrowed"))?;
        let cache = cache.as_mut()
            .ok_or_else(|| RuntimeError::new("Canonical input cache is unavailable"))?;
        unsafe { cache.get(access, id, 16) }
    };
    let (data, item) = if let Some(cached) = cached {
        cached
    } else {
        let property_class = access.native_class("Core.Property")?;
        let mut count = 0usize;
        unsafe { for_each_input_property(class.cast(), property_class, |_| count += 1); }
        let size = mem::size_of::<crate::input::InputVarCache>()
            .checked_add(count.checked_mul(mem::size_of::<*mut c_void>())
                .ok_or_else(|| RuntimeError::new("Canonical input cache size overflow"))?)
            .and_then(|size| i32::try_from(size).ok())
            .ok_or_else(|| RuntimeError::new("Canonical input cache exceeds INT size"))?;
        let (data, item) = {
            let mut cache = state.cache.try_borrow_mut()
                .map_err(|_| RuntimeError::new("Canonical input cache is already borrowed"))?;
            let cache = cache.as_mut()
                .ok_or_else(|| RuntimeError::new("Canonical input cache is unavailable"))?;
            unsafe { cache.create(access, id, size, 16, 0)? }
        };
        let variables = data.cast::<crate::input::InputVarCache>();
        unsafe { (*variables).count = count as i32; }
        let properties = unsafe { ptr::addr_of_mut!((*variables).properties).cast::<*mut c_void>() };
        let mut index = 0;
        unsafe {
            for_each_input_property(class.cast(), property_class, |property| {
                properties.add(index).write(property.cast());
                index += 1;
            });
        }
        (data, item)
    };
    // Binding execution may synchronously reenter Engine/cache operations.
    // Keep the item locked, not the RefCell borrowed, through that execution.
    let result = operation(data.cast());
    let unlock = (|| {
        let mut cache = state.cache.try_borrow_mut()
            .map_err(|_| RuntimeError::new("Canonical input cache is already borrowed"))?;
        let cache = cache.as_mut()
            .ok_or_else(|| RuntimeError::new("Canonical input cache is unavailable"))?;
        unsafe { cache.unlock(access, item) }
    })();
    result.and_then(|value| unlock.map(|()| value))
}

/// Runs canonical `UInput::ReadInput` with the source class-keyed input-variable
/// cache locked across held-key processing and subsequent float-axis scaling.
///
/// # Safety
/// `input` is a live canonical `UInput` whose viewport and actor remain live
/// through synchronous binding execution.
pub(crate) unsafe fn read_input(
    access: CoreAccess<'_>,
    input: *mut UInput,
    delta_seconds: f32,
) -> RuntimeResult<()> {
    if !access.is_running() {
        return Ok(());
    }
    if input.is_null() {
        return Err(RuntimeError::new("Null canonical UInput in ReadInput"));
    }
    let viewport = unsafe { ptr::addr_of!((*input).viewport).read_unaligned().cast::<UViewport>() };
    if viewport.is_null() {
        return Err(RuntimeError::new("Canonical UInput has no UViewport"));
    }
    let actor = unsafe { ptr::addr_of!((*viewport).actor).read_unaligned() };
    if actor.is_null() {
        return Err(RuntimeError::new("Canonical UInput viewport has no actor"));
    }

    let class = unsafe { (*actor.cast::<UObject>()).class };
    let float_class = access.native_class("Core.FloatProperty")?;
    unsafe { with_input_variables(access, class.cast(), |variables| {
        if delta_seconds != -1.0 {
            let category = access.intern_str("Log", FindName::Add)?;
            let mut output = |text: &[u32]| access.emit_log(category, text);
            for key in 0..crate::input::IK_MAX {
                if (*input).key_down_table[key as usize] != 0 {
                    process(
                        access,
                        input,
                        key,
                        crate::input::IST_HOLD,
                        delta_seconds,
                        &mut output,
                    )?;
                }
            }
        }

        let properties = ptr::addr_of!((*variables).properties).cast::<*mut UProperty>();
        let scale = if delta_seconds != -1.0 { 20.0_f32 / delta_seconds } else { 0.0 };
        for index in 0..(*variables).count {
            let property = properties.add(index as usize).read();
            if hp2_core::object_model::is_a(property.cast(), float_class.cast()) {
                let offset = (*property).offset;
                let actor = (*viewport).actor;
                let axis = actor.cast::<u8>().offset(offset as isize).cast::<f32>();
                let value = axis.read_unaligned();
                axis.write_unaligned(value * scale);
            }
        }
        Ok(())
    }) }
}

/// Implements `APlayerPawn::ZeroInputFloats` over Core's reflected class chain.
///
/// # Safety
/// `pawn` is a live canonical AActor whose reflected class metadata and
/// property offsets remain valid for this call.
pub(crate) unsafe fn zero_input_floats(
    access: CoreAccess<'_>,
    pawn: *mut AActor,
) -> RuntimeResult<()> {
    if pawn.is_null() {
        return Err(RuntimeError::new("Null canonical pawn in ZeroInputFloats"));
    }
    let actor = pawn.cast::<UObject>();
    let class = unsafe { ptr::addr_of!((*actor).class).read_unaligned() };
    let property_class = access.native_class("Core.Property")?;
    let float_class = access.native_class("Core.FloatProperty")?;
    let mut structure = class.cast::<UStruct>();
    while !structure.is_null() {
        let mut field = unsafe { ptr::addr_of!((*structure).children).read_unaligned() };
        while !field.is_null() {
            if unsafe { hp2_core::object_model::is_a(field.cast(), property_class.cast()) } {
                let property = field.cast::<UProperty>();
                if unsafe { ptr::addr_of!((*property).property_flags).read_unaligned() } & 0x0000_0004 != 0
                    && unsafe { hp2_core::object_model::is_a(field.cast(), float_class.cast()) }
                {
                    let offset = unsafe { ptr::addr_of!((*property).offset).read_unaligned() };
                    let value = unsafe { pawn.cast::<u8>().wrapping_offset(offset as isize).cast::<f32>() };
                    unsafe { value.write_unaligned(0.0); }
                }
            }
            field = unsafe { ptr::addr_of!((*field.cast::<UField>()).next).read_unaligned() };
        }
        structure = unsafe { ptr::addr_of!((*structure.cast::<hp2_abi::engine::generated::UField>()).super_field).read_unaligned().cast() };
    }
    Ok(())
}

#[cfg(test)]
#[path = "input_owner_set_binding_tests.rs"]
mod set_binding_tests;
#[cfg(test)]
#[path = "input_owner_vm_binding_tests.rs"]
mod vm_binding_tests;
#[cfg(test)]
#[path = "input_owner_gameplay_tests.rs"]
mod gameplay_tests;
#[cfg(test)]
#[path = "input_owner_inversion_tests.rs"]
mod inversion_tests;
#[cfg(test)]
#[path = "input_owner_inversion_vm_tests.rs"]
mod inversion_vm_tests;

#[cfg(test)]
mod render_host_contracts {
    use super::*;
    use hp2_abi::engine::generated::APlayerPawn;

    #[test]
    fn cached_input_scales_only_input_axes_and_survives_eviction_and_reentry() {
        let owner = crate::render_owner::render_test_support::owner("input-variable-cache");
        let access = owner.access();
        let state = access.owner_context::<crate::core_owner::EngineState>().unwrap();
        state.cache.borrow_mut().as_mut().unwrap()
            .initialize_in_place(access, 1024 * 1024, 512, 0).unwrap();
        let actor = crate::render_owner::render_test_support::construct(access, "Engine.Camera")
            .cast::<APlayerPawn>();
        let mut viewport_storage: UViewport = unsafe { mem::zeroed() };
        let viewport = &raw mut viewport_storage;
        let input = crate::render_owner::render_test_support::construct(access, "Engine.Input")
            .cast::<UInput>();
        unsafe {
            (*viewport).actor = actor.cast();
            (*viewport).input = input.cast();
            (*input).viewport = viewport.cast();
            (*actor).a_forward = 2.0;
            (*actor).a_turn = -3.0;
            (*actor.cast::<AActor>()).draw_scale = 1.25;
            access.set_running(true);
            read_input(access, input, 0.5).unwrap();
            assert_eq!((*actor).a_forward, 80.0);
            assert_eq!((*actor).a_turn, -120.0);
            assert_eq!((*actor.cast::<AActor>()).draw_scale, 1.25);

            let class = (*actor.cast::<UObject>()).class;
            let name: Vec<u32> = "aForward".chars().map(u32::from).chain([0]).collect();
            with_input_variables(access, class.cast(), |_| {
                let axis = find_input_property(access, actor.cast(), &name, "FloatProperty")?;
                assert_eq!(axis.cast::<f32>(), ptr::addr_of_mut!((*actor).a_forward));
                axis.cast::<f32>().write(4.0);
                Ok(())
            }).unwrap();
            let failed: RuntimeResult<()> = with_input_variables(access, class.cast(), |_| {
                Err(RuntimeError::new("input binding failure"))
            });
            assert!(failed.is_err());
            state.cache.borrow_mut().as_mut().unwrap().flush(access, 0, u32::MAX, 0).unwrap();
            read_input(access, input, 0.25).unwrap();
            assert_eq!((*actor).a_forward, 320.0);

            read_input(access, input, -1.0).unwrap();
            assert_eq!((*actor).a_forward, 0.0);
            assert_eq!((*actor).a_turn, 0.0);
            let binding: Vec<u32> = "AXIS aForward Speed=2.0".chars().map(u32::from).chain([0]).collect();
            access.assign_fstring(ptr::addr_of_mut!((*input).bindings[10]).cast(), &binding).unwrap();
            (*input).key_down_table[10] = 1;
            read_input(access, input, 0.5).unwrap();
            assert_eq!((*actor).a_forward, 40.0, "held input accumulates before axis scaling");
            (*input).viewport = ptr::null_mut();
            access.set_running(false);
        }
    }
}
