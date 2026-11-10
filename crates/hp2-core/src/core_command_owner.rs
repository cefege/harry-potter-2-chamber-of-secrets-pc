//! Canonical Core hosts for UObject static and USystem command policies.

use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::object_console::{
    self, Hp2ObjectConsoleCommand as Command, Hp2ObjectConsoleLeaf as Leaf,
    Hp2ObjectConsoleLog as LogEvent, Hp2ObjectConsoleRow, Hp2ObjectConsoleServices,
    Hp2ObjectConsoleVisitor,
};
use crate::ansi::{wide, Tchar};
use hp2_abi::engine::generated::{FNameEntry, UClass, UObject, UProperty, UState, ULinkerLoad};
use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr;

fn display(units: &[u32]) -> String {
    units.iter().copied().filter_map(char::from_u32).collect()
}

struct Context<'a, 'out> {
    access: CoreAccess<'a>,
    command: *const Tchar,
    output: &'out mut dyn FnMut(&[u32]) -> RuntimeResult<()>,
    class_name: [Tchar; 256],
    property_name: [Tchar; 256],
    failure: RefCell<Option<RuntimeError>>,
}

impl Context<'_, '_> {
    fn fail(&self, error: RuntimeError) -> i32 {
        let mut failure = self.failure.borrow_mut();
        if failure.is_none() { *failure = Some(error); }
        1
    }

    fn finish<T>(&self, result: RuntimeResult<T>, output: &mut T) -> i32 {
        match result {
            Ok(value) => { *output = value; 0 }
            Err(error) => self.fail(error),
        }
    }

    fn failure(&self, operation: &str, status: i32) -> RuntimeError {
        self.failure.borrow_mut().take().unwrap_or_else(|| {
            RuntimeError::new(&format!("Canonical UObject::StaticExec {operation} failed ({status})"))
        })
    }

    unsafe fn token(&mut self, index: usize) -> bool {
        let dst = if index == 0 { self.class_name.as_mut_ptr() } else { self.property_name.as_mut_ptr() };
        unsafe { crate::tokenize::parse_token(&mut self.command, dst, 256, true) }
    }

    fn name_units(&self, index: usize) -> RuntimeResult<Vec<u32>> {
        let text = if index == 0 { &self.class_name } else { &self.property_name };
        Ok(text.iter().copied().take_while(|unit| *unit != 0).map(|unit| unit as u32).collect())
    }
}


fn call_result(opaque: *mut c_void, result: RuntimeResult<usize>, output: *mut usize) -> i32 {
    let state = unsafe { &*opaque.cast::<Context<'_, '_>>() };
    let mut value = 0;
    let status = state.finish(result, &mut value);
    if status == 0 { unsafe { output.write(value) }; }
    status
}

unsafe extern "C" fn command(opaque: *mut c_void, operation: Command, result: *mut i32) -> i32 {
    let state = unsafe { &mut *opaque.cast::<Context<'_, '_>>() };
    macro_rules! parsed {
        ($literal:literal) => {
            unsafe { crate::command::parse_command(&mut state.command, wide($literal).as_ptr()) != 0 }
        };
    }
    let matched = match operation {
        Command::Mem => parsed!(b"MEM\0"), Command::ResetProfile => parsed!(b"RESETPROFILE\0"),
        Command::DumpNatives => parsed!(b"DUMPNATIVES\0"), Command::Get => parsed!(b"GET\0"),
        Command::Set => parsed!(b"SET\0"), Command::Obj => parsed!(b"OBJ\0"),
        Command::Garbage => parsed!(b"GARBAGE\0"), Command::Mark => parsed!(b"MARK\0"),
        Command::MarkCheck => parsed!(b"MARKCHECK\0"), Command::Refs => parsed!(b"REFS\0"),
        Command::Hash => parsed!(b"HASH\0"), Command::Classes => parsed!(b"CLASSES\0"),
        Command::Dependencies => parsed!(b"DEPENDENCIES\0"), Command::List => parsed!(b"LIST\0"),
        Command::VfHash => parsed!(b"VFHASH\0"), Command::Linkers => parsed!(b"LINKERS\0"),
        Command::GTime => parsed!(b"GTIME\0"), Command::Verbose => parsed!(b"VERBOSE\0"),
    };
    unsafe { result.write(i32::from(matched)) };
    0
}

unsafe extern "C" fn call(
    opaque: *mut c_void, operation: Leaf, a: usize, b: usize, output: *mut usize,
) -> i32 {
    let state = unsafe { &mut *opaque.cast::<Context<'_, '_>>() };
    let access = state.access;
    let result = (|| -> RuntimeResult<usize> {
        use Leaf as L;
        match operation {
            L::ProfileEnabled => Ok(usize::from(hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.slow_checks != 0)),
            L::DumpAllocs => {
                let category = access.intern_str("Exit", crate::name_owner::FindName::Find)?;
                let message = (*b"Allocation checking disabled").map(u32::from);
                access.emit_log(category, &message)?;
                Ok(0)
            }
            L::ObjectLen => Ok(access.object_count() as usize),
            L::ObjectAt => Ok(access.object_at(a as i32) as usize),
            L::IsKind => {
                let name = match b { 0 => "Core.Class", 1 => "Core.Function", _ => "Core.State" };
                let class = access.native_class(name)?;
                Ok(usize::from(unsafe { crate::object_model::is_a((a as *mut UObject).cast(), class.cast()) }))
            }
            L::ResetFunction => Ok(0),
            L::ResetTicks => {
                unsafe { crate::timing_globals::GTicks = 1; }
                Ok(0)
            }
            L::Token => { let found = unsafe { state.token(a) }; Ok(usize::from(found)) }
            L::FindClass => {
                let name = state.name_units(0)?;
                let metaclass = access.native_class("Core.Class")?;
                let mut found = unsafe { access.find_object_by_name(metaclass, &name)? };
                // Stock scripts also use ini:Package.Class as a direct class
                // selector. Preserve configured aliases, including broken or
                // empty aliases: fall back only if the key does not exist.
                // Restrict the fallback to one package separator and loaded
                // classes, never a longer config alias or a package load.
                let prefix = b"ini:";
                if found.is_none() && name.len() < 1024 && name.len() > 4
                    && name[..4].iter().zip(prefix).all(|(&unit, &byte)| unit <= 127 && (unit as u8).eq_ignore_ascii_case(&byte))
                {
                    let direct = &name[4..];
                    let mut separators = direct.iter().enumerate().filter(|(_, unit)| **unit == u32::from('.'));
                    let separator = separators.next().map(|(index, _)| index);
                    if separator.is_some_and(|index| index > 0 && index + 1 < direct.len())
                        && separators.next().is_none()
                        && crate::core_reflection::resolve_configured_load_name(access, &name)?.is_none()
                    {
                        found = unsafe { access.find_object_by_name(metaclass, direct)? };
                    }
                }
                Ok(found.unwrap_or(ptr::null_mut()) as usize)
            }
            L::FindProperty => {
                let name = access.intern(&state.name_units(1)?, crate::name_owner::FindName::Find)?;
                let property_class = access.native_class("Core.Property")?;
                Ok(unsafe { access.find_field((a as *mut UClass).cast(), name, property_class).cast::<UProperty>() } as usize)
            }
            L::ExportDefault => {
                let class = a as *mut UClass;
                let property = b as *mut UProperty;
                let defaults = unsafe { &(*class).defaults };
                let text = if defaults.num > 0 {
                    let mut value = Vec::new();
                    let data = defaults.data.cast::<u8>().wrapping_offset(unsafe { (*property).offset } as isize);
                    unsafe { access.property_export_raw_value(property, data, data, 1, &mut value)?; }
                    value
                } else { Vec::new() };
                (state.output)(&text)?;
                Ok(0)
            }
            L::SkipSpaces => {
                while unsafe { *state.command } == b' ' as Tchar { state.command = unsafe { state.command.add(1) }; }
                Ok(0)
            }
            L::SetProperty => {
                let class = a as *mut UClass;
                let property = b as *mut UProperty;
                let input = unsafe { core_text(state.command) };
                let selector = display(&state.name_units(0)?);
                if selector.to_ascii_lowercase().starts_with("ini:")
                    && is_menu_config_property(access, class, property)? {
                    unsafe { set_menu_config_property(access, class, property, &input)?; }
                } else if selector.to_ascii_lowercase().starts_with("ini:")
                    && is_stock_actor_config_property(access, class, property)? {
                    unsafe { set_stock_actor_config_property(access, class, property, &input)?; }
                } else {
                    let defaults = unsafe { &mut (*class).defaults };
                    if defaults.num <= 0 { return Err(RuntimeError::new("Class has no default property storage")); }
                    let value = defaults.data.cast::<u8>().wrapping_offset(unsafe { (*property).offset } as isize);
                    unsafe { access.property_import_raw_value(property, value, &input, ptr::null_mut(), 1)?; }
                }
                Ok(0)
            }
            L::NoGc => Ok(access.no_gc() as usize),
            L::SetNoGc => { access.set_no_gc(a as i32); Ok(0) }
            L::CollectGarbage => {
                let keep = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.native
                    | if access.is_editor() {
                        crate::class_metadata::METADATA_RF_PUBLIC_STANDALONE
                            & !hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.public_flag
                    } else { 0 };
                let sink = RefCell::new(&mut *state.output);
                access.collect_garbage(keep, access.no_gc(), &|event| {
                    let text = match event {
                        crate::core_gc::GcLog::Collecting => "Collecting garbage".to_owned(),
                        crate::core_gc::GcLog::PurgeBegin { no_gc } => format!("Purging garbage (NoGC={no_gc})"),
                        crate::core_gc::GcLog::GarbageCollected { index, object } => format!("Garbage {index}: {}", display(&object)),
                        crate::core_gc::GcLog::NameDeleted { index, name } => format!("Deleted name {index}: {}", display(&name)),
                        crate::core_gc::GcLog::Counts { before, remaining, references } => format!("{before} Objects, {remaining} Remaining, {references} References"),
                        crate::core_gc::GcLog::Referenced { object, context } => format!("Referenced {}{}", display(&object), context.as_deref().map(|name| format!(" from {}", display(name))).unwrap_or_default()),
                        crate::core_gc::GcLog::Marked { object, context } => format!("Marked {}{}", display(&object), context.as_deref().map(|name| format!(" from {}", display(name))).unwrap_or_default()),
                        crate::core_gc::GcLog::SerializeRouteFailure { object } => format!("Unable to trace references from {}", display(&object)),
                        crate::core_gc::GcLog::ValidationNull => "Null object in object table".to_owned(),
                        crate::core_gc::GcLog::ValidationInvalidIndex { index, object } => format!("Invalid object index {index}: {}", display(&object)),
                        crate::core_gc::GcLog::ValidationEmptySlot { index, object } => format!("Empty object slot {index}: {}", display(&object)),
                        crate::core_gc::GcLog::ValidationOtherObject { index, object, other } => format!("Object index {index} has {} instead of {}", display(&object), display(&other)),
                    };
                    (sink.borrow_mut())(&text.chars().map(u32::from).collect::<Vec<_>>())
                })?;
                Ok(0)
            }
            L::Flags => Ok(unsafe { (*((a as *mut UObject))).object_flags as usize }),
            L::SetFlags => { unsafe { (*((a as *mut UObject))).object_flags |= b as u32; } Ok(0) }
            L::ClearFlags => { unsafe { (*((a as *mut UObject))).object_flags &= !(b as u32); } Ok(0) }
            L::ParseObject => parse_object(access, state, a, b),
            L::ObjectClass => Ok(unsafe { (*((a as *mut UObject))).class as usize }),
            L::Outer => Ok(unsafe { (*((a as *mut UObject))).outer as usize }),
            L::IsIn => {
                let mut object = a as *mut UObject;
                while !object.is_null() && object != b as *mut UObject { object = unsafe { (*object).outer.cast() }; }
                Ok(usize::from(object == b as *mut UObject))
            }
            L::PendingKill => Ok(0),
            L::CountMem => {
                let mapping = CountingMap;
                let archive = crate::object_archive::ObjectArchive::memory_count(&mapping);
                unsafe { access.serialize_object(a as *mut UObject, &archive)?; }
                let (num, max) = archive.counted_bytes();
                let row = unsafe { &mut *(b as *mut Hp2ObjectConsoleRow) };
                row.num = num;
                row.max = max;
                Ok(0)
            }
            L::SortObjects => {
                let rows = unsafe { std::slice::from_raw_parts_mut(a as *mut Hp2ObjectConsoleRow, b) };
                rows.sort_by(|left, right| compare_names(access, left.object as *mut UObject, right.object as *mut UObject));
                Ok(0)
            }
            L::SortClasses => {
                let rows = unsafe { std::slice::from_raw_parts_mut(a as *mut Hp2ObjectConsoleRow, b) };
                rows.sort_by(|left, right| object_console::compare_max(left.max, right.max).cmp(&0));
                Ok(0)
            }
            L::ClassCompare => {
                let left = unsafe { access.name_text(i32::from_ne_bytes((*(*(a as *mut UObject)).class.cast::<UObject>()).name))? };
                let right = unsafe { access.name_text(i32::from_ne_bytes((*(*(b as *mut UObject)).class.cast::<UObject>()).name))? };
                Ok((left.cmp(&right) as i32) as usize)
            }
            L::NameNumber => {
                let object = a as *mut UObject;
                let class = unsafe { (*object).class.cast::<UObject>() };
                let name = access.name_text(i32::from_ne_bytes(unsafe { (*object).name }))?;
                let class_name = access.name_text(i32::from_ne_bytes(unsafe { (*class).name }))?;
                let suffix = name.get(class_name.len()..).unwrap_or_default();
                let value = display(suffix).parse::<i32>().unwrap_or(0);
                Ok(value as usize)
            }
            L::ObjectStaticClass => Ok(access.native_class("Core.Object")? as usize),
            L::DisplayHash => {
                let (names, used, bins) = access.name_hash_stats();
                let text = format!("Hash: {names} names, {used}/{bins} hash bins");
                (state.output)(&text.chars().map(u32::from).collect::<Vec<_>>())?;
                Ok(0)
            }
            L::ExcludePackage => exclude_package(access, state, a),
            L::SuperClass => Ok(unsafe { (*((a as *mut UClass))).super_field as usize }),
            L::Serialize => {
                let archive = unsafe { &*(b as *const crate::object_archive::ObjectArchive<'_>) };
                unsafe { access.serialize_object(a as *mut UObject, archive)?; }
                Ok(0)
            }
            L::HashCount => Ok(256),
            L::HashHead => Ok(unsafe { (*((a as *mut UState))).vf_hash[b] as usize }),
            L::HashNext => Ok(unsafe { (*((a as *mut UObject))).hash_next as usize }),
            L::LoaderLen | L::LoaderAt | L::NameLen | L::NameSize => {
                linker_leaf(access, operation, a, b)
            }
            L::LinkerRow => {
                let linker = a as *mut ULinkerLoad;
                let raw = unsafe { &(*linker).base };
                let filename = if raw.filename.num <= 0 { String::new() } else {
                    display(unsafe { std::slice::from_raw_parts(raw.filename.data.cast::<u32>(), raw.filename.num as usize) }
                        .iter().copied().take_while(|unit| *unit != 0).collect::<Vec<_>>().as_slice())
                };
                let root = format_object(access, raw.linker_root.cast::<UObject>());
                let names = raw.name_map.num;
                let name_bytes = names as usize * std::mem::size_of::<hp2_abi::engine::generated::FName>();
                let imports = raw.import_map.num;
                let import_bytes = imports as usize * std::mem::size_of::<hp2_abi::engine::generated::FObjectImport>();
                let exports = raw.export_map.num;
                let export_bytes = exports as usize * std::mem::size_of::<hp2_abi::engine::generated::FObjectExport>();
                let generations = raw.summary.generations.num;
                let lazy = unsafe { (*linker).lazy_loaders.num };
                let row = format!(
                    "{} ({}): Names={} ({}K/{}K) Imports={} ({}K) Exports={} ({}K) Gen={} Lazy={}",
                    filename, root, names, name_bytes / 1024, b / 1024, imports,
                    import_bytes / 1024, exports, export_bytes / 1024, generations, lazy,
                );
                (state.output)(&row.chars().map(u32::from).collect::<Vec<_>>())?;
                Ok(0)
            }
        }
    })();
    call_result(opaque, result, output)
}

fn is_stock_actor_config_property(access: CoreAccess<'_>, class: *mut UClass, property: *mut UProperty) -> RuntimeResult<bool> {
    let path = object_path(access, class.cast());
    let name = display(&access.name_text(i32::from_ne_bytes(unsafe { (*property.cast::<UObject>()).name }))?);
    Ok((path.eq_ignore_ascii_case("HGame.Harry") && ["bAutoCenterCamera", "bAutoQuaff", "bMoveWhileCasting", "ObjectDetail", "Difficulty"].iter().any(|item| name.eq_ignore_ascii_case(item)))
        || (path.eq_ignore_ascii_case("Engine.PlayerPawn") && name.eq_ignore_ascii_case("bInvertMouse")))
}

/// Stage a scalar SET value. Stock scripts send enum indices as decimal text.
/// The legacy enum ImportText cursor consumes an unknown name before trying a
/// number, so recognize numeric byte tokens here without changing ImportText's
/// existing behavior for other callers. The caller checks the reflected range.
unsafe fn import_staged_command_value(access: CoreAccess<'_>, property: *mut UProperty, kind: crate::core_class_specs::NativeRecordKind, staged: &mut u32, input: &[u32]) -> RuntimeResult<usize> {
    use crate::core_class_specs::NativeRecordKind as K;
    if kind == K::ByteProperty && input.first().is_some_and(|c| (48..=57).contains(c)) {
        let end = input.iter().take_while(|c| (48..=57).contains(*c)).count();
        let digits: String = input[..end].iter().filter_map(|&unit| char::from_u32(unit)).collect();
        *staged = u32::from(digits.parse::<u8>().map_err(|_| RuntimeError::new("Invalid byte property value"))?);
        return Ok(end);
    }
    unsafe { access.property_import_raw_value(property, (staged as *mut u32).cast(), input, ptr::null_mut(), 1) }
}

/// Stages the stock actor globalconfig values. Only actor implementations with
/// the canonical Actor edit hook and nontransactional objects are admitted:
/// that hook has no fallible callback in this state. All reflected descendants'
/// defaults share the declaring class's globalconfig section.
unsafe fn set_stock_actor_config_property(access: CoreAccess<'_>, class: *mut UClass, property: *mut UProperty, input: &[u32]) -> RuntimeResult<()> {
    use crate::core_class_specs::NativeRecordKind as K;
    let flags = unsafe { (*property).property_flags };
    if flags & 0x44000 != 0x44000 || unsafe { (*class).class_flags } & 4 == 0 || unsafe { (*property).array_dim } != 1 {
        return Err(RuntimeError::new("Expected scalar globalconfig property"));
    }
    let kind = unsafe { access.record_kind(property.cast())? };
    let (size, mask) = match kind {
        K::BoolProperty => (4, unsafe { (*property.cast::<hp2_abi::engine::generated::UBoolProperty>()).bit_mask as u32 }),
        K::ByteProperty => (1, 0),
        _ => return Err(RuntimeError::new("Expected stock bool or enum property")),
    };
    let offset = unsafe { (*property).offset };
    if offset < 0 || unsafe { (*property).element_size } != size || (size == 4 && mask == 0) {
        return Err(RuntimeError::new("Invalid actor property layout"));
    }
    let owner = unsafe { (*property.cast::<UObject>()).outer.cast::<UClass>() };
    if owner.is_null() || !unsafe { crate::object_model::is_child_of(class.cast(), owner.cast()) } {
        return Err(RuntimeError::new("Invalid globalconfig declaring class"));
    }
    let text = input.iter().copied().map(|c| char::from_u32(c).ok_or_else(|| RuntimeError::new("Invalid property text"))).collect::<RuntimeResult<String>>()?;
    if size == 1 && text.trim().bytes().all(|byte| byte.is_ascii_digit()) && text.trim().parse::<u8>().is_err() {
        return Err(RuntimeError::new("Invalid enum value"));
    }
    let selected_default = unsafe { access.default_object(class)? };
    let selected_value = unsafe { access.property_address(selected_default, property)? };
    let mut staged = if size == 1 { u32::from(unsafe { selected_value.read() }) } else { unsafe { selected_value.cast::<u32>().read_unaligned() } };
    let consumed = unsafe { import_staged_command_value(access, property, kind, &mut staged, input)? };
    if consumed == 0 || consumed > input.len() || input[consumed..].iter().any(|&c| !matches!(c, 32 | 9 | 10 | 13)) {
        let name = display(&access.name_text(i32::from_ne_bytes(unsafe { (*property.cast::<UObject>()).name }))?);
        return Err(RuntimeError::new(&format!("Invalid or incomplete property value: SET ini:{} {} {:?} (consumed {} of {})", object_path(access, class.cast()), name, text, consumed, input.len())));
    }
    if size == 1 {
        let enumeration = unsafe { (*property.cast::<hp2_abi::engine::generated::UByteProperty>()).r#enum.cast::<hp2_abi::engine::generated::UEnum>() };
        if enumeration.is_null() || staged >= unsafe { (*enumeration).names.num.max(0) as u32 } {
            return Err(RuntimeError::new("Value is outside its reflected enum"));
        }
    }
    let config_name = unsafe { (*class).class_config_name };
    let actor = access.native_class("Engine.Actor")?;
    let actor_default = unsafe { access.default_object(actor)? };
    let actor_hook = unsafe { access.class_ops(actor_default)? }.post_edit_change;
    let class_meta = access.native_class("Core.Class")?;
    let mut defaults = Vec::new();
    let mut objects = Vec::new();
    for index in 0..access.object_count() {
        let object = access.object_at(index);
        if object.is_null() { continue; }
        if unsafe { crate::object_model::is_a(object.cast(), class_meta.cast()) } {
            let candidate = object.cast::<UClass>();
            if unsafe { crate::object_model::is_child_of(candidate.cast(), owner.cast()) }
                && unsafe { (*candidate).class_flags } & 4 != 0
                && unsafe { (*candidate).class_config_name } == config_name
                && unsafe { (*candidate).defaults.num } > 0 {
                if offset.checked_add(size).is_none_or(|end| end > unsafe { (*candidate).properties_size }) {
                    return Err(RuntimeError::new("Inherited globalconfig offset is out of bounds"));
                }
                let default = unsafe { access.default_object(candidate)? };
                defaults.push(unsafe { access.property_address(default, property)? });
            }
        } else if unsafe { crate::object_model::is_a(object.cast(), class.cast()) } {
            let hook = unsafe { access.class_ops(object)? }.post_edit_change;
            // SpawnActor marks runtime actors transactional even when no
            // transaction manager exists. In that normal state Modify is a
            // no-op; only an installed manager can add a fallible side effect.
            if (unsafe { (*object).object_flags } & 1 != 0 && access.has_object_transactions())
                || !std::ptr::fn_addr_eq(hook, actor_hook)
            {
                return Err(RuntimeError::new("Cannot safely notify this actor"));
            }
            objects.push((object, unsafe { access.property_address(object, property)? }));
        }
    }
    let mut persisted = Vec::new();
    unsafe { access.property_export_raw_value(property, (&mut staged as *mut u32).cast(), ptr::null_mut(), 0, &mut persisted)?; }
    let filename = access.name_text(i32::from_ne_bytes(config_name))?;
    let section = object_path(access, owner.cast()).chars().map(u32::from).collect::<Vec<_>>();
    let name = access.name_text(i32::from_ne_bytes(unsafe { (*property.cast::<UObject>()).name }))?;
    unsafe { &mut *access.config_storage() }.publish_one(Some(&filename), &section, &name, &persisted)?;
    unsafe fn assign(address: *mut u8, value: u32, size: i32, mask: u32) {
        if size == 1 { unsafe { address.write(value as u8) }; }
        else {
            let old = unsafe { address.cast::<u32>().read_unaligned() };
            unsafe { address.cast::<u32>().write_unaligned((old & !mask) | (value & mask)) };
        }
    }
    for address in defaults { unsafe { assign(address, staged, size, mask); } }
    for (object, address) in objects { unsafe { assign(address, staged, size, mask); access.post_edit_change(object)?; } }
    Ok(())
}

fn is_menu_config_property(access: CoreAccess<'_>, class: *mut UClass, property: *mut UProperty) -> RuntimeResult<bool> {
    let class_path = object_path(access, class.cast());
    let name = display(&access.name_text(i32::from_ne_bytes(unsafe { (*property.cast::<UObject>()).name }))?);
    Ok((class_path.eq_ignore_ascii_case("ALAudio.ALAudioSubsystem")
        && (name.eq_ignore_ascii_case("MusicVolume") || name.eq_ignore_ascii_case("SoundVolume")))
        || (class_path.eq_ignore_ascii_case("SDLDrv.SDLClient")
            && (name.eq_ignore_ascii_case("Brightness") || name.eq_ignore_ascii_case("TextureDetail"))))
}

/// Transactional support for the stock sound/video menu. These exact native
/// classes have known PostEditChange behavior: SDLClient uses Object::Modify;
/// ALAudio additionally clamps both volume fields to [0,1]. Transactional
/// objects and derived classes are rejected because their callbacks can reenter
/// or fail after publication. No callback is invoked before the file is saved.
unsafe fn set_menu_config_property(access: CoreAccess<'_>, class: *mut UClass, property: *mut UProperty, input: &[u32]) -> RuntimeResult<()> {
    use crate::core_class_specs::NativeRecordKind as K;
    let class_path = object_path(access, class.cast());
    let property_name = access.name_text(i32::from_ne_bytes(unsafe { (*property.cast::<UObject>()).name }))?;
    let property_text = display(&property_name);
    let audio = class_path.eq_ignore_ascii_case("ALAudio.ALAudioSubsystem");
    let client = class_path.eq_ignore_ascii_case("SDLDrv.SDLClient");
    let is_float = (audio && (property_text.eq_ignore_ascii_case("MusicVolume") || property_text.eq_ignore_ascii_case("SoundVolume")))
        || (client && property_text.eq_ignore_ascii_case("Brightness"));
    let is_enum = client && property_text.eq_ignore_ascii_case("TextureDetail");
    if !is_float && !is_enum {
        return Err(RuntimeError::new("Transactional ini SET does not support this class and property"));
    }
    let flags = unsafe { (*property).property_flags };
    if flags & crate::object_config::CPF_CONFIG_FLAG == 0 || flags & 0x40000 != 0
        || unsafe { (*class).class_flags } & 4 == 0 || unsafe { (*property).array_dim } != 1 {
        return Err(RuntimeError::new("Unsupported config property metadata"));
    }
    let kind = unsafe { access.record_kind(property.cast())? };
    let size = if is_float && kind == K::FloatProperty { 4 } else if is_enum && kind == K::ByteProperty { 1 }
        else { return Err(RuntimeError::new("Unexpected menu property type")); };
    let offset = unsafe { (*property).offset };
    if unsafe { (*property).element_size } != size || offset < 0
        || offset.checked_add(size).is_none_or(|end| end > unsafe { (*class).properties_size }) {
        return Err(RuntimeError::new("Invalid menu property layout"));
    }
    let text = input.iter().copied().map(|unit| char::from_u32(unit).ok_or_else(|| RuntimeError::new("Invalid property text"))).collect::<RuntimeResult<String>>()?;
    if is_float {
        let number = text.trim().parse::<f32>().map_err(|_| RuntimeError::new("Invalid floating-point property value"))?;
        if !number.is_finite() || !(0.0..=1.0).contains(&number) {
            return Err(RuntimeError::new("Menu property value must be between 0 and 1"));
        }
    } else if text.trim().bytes().all(|byte| byte.is_ascii_digit()) && text.trim().parse::<u8>().is_err() {
        return Err(RuntimeError::new("Invalid texture detail value"));
    }
    let defaults = unsafe { access.default_object(class)? };
    let default_value = unsafe { access.property_address(defaults, property)? };
    let mut staged: u32 = if size == 1 { u32::from(unsafe { default_value.read() }) } else { unsafe { default_value.cast::<u32>().read_unaligned() } };
    let consumed = unsafe { import_staged_command_value(access, property, kind, &mut staged, input)? };
    if consumed == 0 || consumed > input.len() || input[consumed..].iter().any(|&c| !matches!(c, 32 | 9 | 10 | 13)) {
        return Err(RuntimeError::new(&format!("Invalid or incomplete property value: SET ini:{} {} {:?} (consumed {} of {})", class_path, property_text, text, consumed, input.len())));
    }
    if is_enum {
        let enumeration = unsafe { (*property.cast::<hp2_abi::engine::generated::UByteProperty>()).r#enum.cast::<hp2_abi::engine::generated::UEnum>() };
        if enumeration.is_null() || staged >= unsafe { (*enumeration).names.num.max(0) as u32 } {
            return Err(RuntimeError::new("Texture detail is outside its reflected enum"));
        }
    }
    let mut objects = Vec::new();
    for index in 0..access.object_count() {
        let object = access.object_at(index);
        if object.is_null() || !unsafe { crate::object_model::is_a(object.cast(), class.cast()) } { continue; }
        if unsafe { (*object).class } != class.cast() || unsafe { (*object).object_flags } & 1 != 0 {
            return Err(RuntimeError::new("Cannot safely notify a derived or transactional object"));
        }
        if audio {
            let audio = object.cast::<hp2_abi::engine::generated::UALAudioSubsystem>();
            let music = unsafe { ptr::addr_of!((*audio).music_volume).read_unaligned() };
            let sound = unsafe { ptr::addr_of!((*audio).sound_volume).read_unaligned() };
            if !music.is_finite() || !sound.is_finite() || !(0.0..=1.0).contains(&music) || !(0.0..=1.0).contains(&sound) {
                return Err(RuntimeError::new("Audio notification would modify another out-of-range volume"));
            }
        }
        objects.push((object, unsafe { access.property_address(object, property)? }));
    }
    let mut persisted = Vec::new();
    unsafe { access.property_export_raw_value(property, (&mut staged as *mut u32).cast(), ptr::null_mut(), 0, &mut persisted)?; }
    let filename = access.name_text(i32::from_ne_bytes(unsafe { (*class).class_config_name }))?;
    let section: Vec<u32> = class_path.chars().map(u32::from).collect();
    unsafe { &mut *access.config_storage() }.publish_one(Some(&filename), &section, &property_name, &persisted)?;
    unsafe fn assign(address: *mut u8, value: u32, size: i32) {
        if size == 1 { unsafe { address.write(value as u8) }; }
        else { unsafe { address.cast::<u32>().write_unaligned(value) }; }
    }
    unsafe { assign(default_value, staged, size); }
    for (object, address) in objects {
        unsafe { assign(address, staged, size); access.post_edit_change(object)?; }
    }
    Ok(())
}

unsafe fn core_text(mut text: *const Tchar) -> Vec<u32> {
    let mut value = Vec::new();
    while unsafe { *text } != 0 { value.push(unsafe { *text } as u32); text = unsafe { text.add(1) }; }
    value
}

fn key_matches(command: *const Tchar, key: &[u32]) -> bool {
    let mut converted = [0 as Tchar; 32];
    if key.len() >= converted.len() { return false; }
    for (destination, source) in converted.iter_mut().zip(key) {
        *destination = *source as Tchar;
    }
    unsafe { crate::ansi::strnicmp(command, converted.as_ptr(), key.len() as i32) == 0 }
}

fn parse_object(access: CoreAccess<'_>, state: &mut Context<'_, '_>, mode: usize, class: usize) -> RuntimeResult<usize> {
    let key: &[u8] = match mode {
        0 => b"CLASS=\0", 1 => b"PACKAGE=\0", 2 => b"INSIDE=\0", 3 => b"NAME=\0",
        _ => return Ok(0),
    };
    let key: Vec<Tchar> = key.iter().copied().map(Tchar::from).collect();
    let mut value = [0; 256];
    // SAFETY: text_parse searches the full NUL-terminated command suffix and
    // writes into the fixed local TCHAR buffer.
    if !unsafe {
        crate::text_parse::parse_string(state.command, key.as_ptr(), value.as_mut_ptr(), value.len() as i32)
    } {
        return Ok(0);
    }
    let name = value.iter().copied().take_while(|unit| *unit != 0).map(|unit| unit as u32).collect::<Vec<_>>();
    let class = match mode {
        0 => access.native_class("Core.Class")?,
        1 | 2 => access.native_class("Core.Package")?,
        _ => class as *mut UClass,
    };
    Ok(unsafe { access.find_object_by_name(class, &name)? }.unwrap_or(ptr::null_mut()) as usize)
}

fn linker_leaf(access: CoreAccess<'_>, operation: Leaf, a: usize, b: usize) -> RuntimeResult<usize> {
    use Leaf as L;
    match operation {
        L::LoaderLen => Ok(access.loader_count() as usize),
        L::LoaderAt => Ok(access.loader_at(a as i32) as usize),
        L::NameLen => Ok(unsafe { (*((a as *mut ULinkerLoad))).base.name_map.num as usize }),
        L::NameSize => {
            let linker = a as *mut ULinkerLoad;
            let map = unsafe { &(*linker).base.name_map };
            let name = unsafe { *(map.data.cast::<hp2_abi::engine::generated::FName>().add(b)) };
            if name.index == hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.none { return Ok(0); }
            let text = access.name_text(name.index)?;
            let length = text.len().min(127);
            Ok((std::mem::size_of::<FNameEntry>() - (128 - length - 1) * std::mem::size_of::<Tchar>()) as usize)
        }
        _ => unreachable!(),
    }
}
unsafe extern "C" fn log_event(opaque: *mut c_void, event: LogEvent, row: *const Hp2ObjectConsoleRow) -> i32 {
    let state = unsafe { &mut *opaque.cast::<Context<'_, '_>>() };
    let row = unsafe { &*row };
    let object = row.object as *mut UObject;
    if matches!(event, LogEvent::Marking | LogEvent::Unmarked | LogEvent::DebugObject) {
        let text = match event {
            LogEvent::Marking => "Marking objects".to_owned(),
            LogEvent::Unmarked => "Unmarked objects:".to_owned(),
            _ => format_object(state.access, object),
        };
        let units = text.chars().map(u32::from).collect::<Vec<_>>();
        return match state.access.emit_log(hp2_abi::engine::generated::CORE_NAME_LOG as i32, &units) {
            Ok(()) => 0,
            Err(error) => state.fail(error),
        };
    }
    if matches!(event, LogEvent::Time) {
        let fixed_time = unsafe { crate::timing_globals::GTempTime };
        let seconds = crate::time::get_float(fixed_time) as f64;
        let text = format!("GTime = {:.6}", seconds);
        let units = text.chars().map(u32::from).collect::<Vec<_>>();
        return match state.access.emit_log(hp2_abi::engine::generated::CORE_NAME_LOG as i32, &units) {
            Ok(()) => 0,
            Err(error) => state.fail(error),
        };
    }
    let text = match event {
        LogEvent::ClassWarning => format!("Unrecognized class {}", display(&state.name_units(0).unwrap_or_default())),
        LogEvent::PropertyWarning => format!("Unrecognized property {}", display(&state.name_units(1).unwrap_or_default())),
        LogEvent::Marking | LogEvent::Unmarked | LogEvent::DebugObject => unreachable!(),
        LogEvent::Blank => String::new(), LogEvent::Space => " ".to_owned(),
        LogEvent::Referencers => format!("Referencers of {}:", format_object(state.access, object)),
        LogEvent::Referencer => format!("   {}", format_object(state.access, object)),
        LogEvent::Reachability => format!("Shortest reachability from root to {}:", format_object(state.access, object)),
        LogEvent::Route => format!("   {}{}", format_object(state.access, object), if row.count == 0 { "" } else if unsafe { (*object).object_flags & hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.native } != 0 { " (native)" } else { " (root)" }),
        LogEvent::Unrooted => "   (Object is not currently rooted)".to_owned(),
        LogEvent::Dependencies => format!("Dependencies of {}:", object_path(state.access, object)),
        LogEvent::ReferenceHeading => format!("   {} references:", format_object(state.access, object)),
        LogEvent::Reference => format!("      {}", format_object(state.access, object)),
        LogEvent::ClassTree => format!("{}{}", " ".repeat(row.count.max(0) as usize), object_name(state.access, object)),
        LogEvent::Objects => "Objects:".to_owned(), LogEvent::ObjectColumns => format!("{:<78} {:>10} {:>10}", "Object", "NumBytes", "MaxBytes"),
        LogEvent::ObjectRow => format!("{:<78} {:>10} {:>10} ", format_object(state.access, object), row.num, row.max),
        LogEvent::ClassColumns => format!("{:<30} {:>6} {:>10} {:>10}", "Class", "Count", "NumKBytes", "MaxKBytes"),
        LogEvent::ClassRow => format!("{:<30} {:>6} {:>10} {:>10}", object_name(state.access, object), row.count, row.num / 1024, row.max / 1024),
        LogEvent::Total => format!("{} Objects ({:.3}M / {:.3}M)", row.count, row.num as f32 as f64 / 1048576.0, row.max as f32 as f64 / 1048576.0),
        LogEvent::Deleted => format!("{} Deleted Objects", row.count), LogEvent::Hashes => "Class VfHashes:".to_owned(),
        LogEvent::StateName => format!("{}:", object_name(state.access, object)),
        LogEvent::HashBucket => format!("   {}: {}", row.object, row.count), LogEvent::Linkers => "Linkers:".to_owned(),
        LogEvent::Time => unreachable!(),
    };
    let units = text.chars().map(u32::from).collect::<Vec<_>>();
    match (state.output)(&units) { Ok(()) => 0, Err(error) => state.fail(error) }
}

fn object_path(access: CoreAccess<'_>, object: *mut UObject) -> String {
    let mut chain = Vec::new();
    let mut current = object;
    while !current.is_null() {
        let name = i32::from_ne_bytes(unsafe { (*current).name });
        chain.push(access.name_text(name).unwrap_or_default());
        current = unsafe { (*current).outer.cast::<UObject>() };
    }
    chain.reverse();
    let mut path = Vec::new();
    for (index, name) in chain.into_iter().enumerate() {
        if index != 0 { path.push('.' as u32); }
        path.extend(name);
    }
    display(&path)
}
fn compare_names(access: CoreAccess<'_>, a: *mut UObject, b: *mut UObject) -> std::cmp::Ordering {
    let class_name = |object: *mut UObject| -> Vec<u32> {
        let class = unsafe { (*object).class.cast::<UObject>() };
        access.name_text(i32::from_ne_bytes(unsafe { (*class).name })).unwrap_or_default()
    };
    let order = class_name(a).cmp(&class_name(b));
    if order != std::cmp::Ordering::Equal { return order; }
    let number = |object: *mut UObject| -> i32 {
        let object_name = access.name_text(i32::from_ne_bytes(unsafe { (*object).name })).unwrap_or_default();
        let class_name = class_name(object);
        display(object_name.get(class_name.len()..).unwrap_or_default()).parse::<i32>().unwrap_or(0)
    };
    number(a).wrapping_sub(number(b)).cmp(&0)
}
fn format_object(access: CoreAccess<'_>, object: *mut UObject) -> String {
    unsafe { access.object_full_name(object) }.map(|units| display(&units)).unwrap_or_else(|_| "<invalid object>".to_owned())
}
fn object_name(access: CoreAccess<'_>, object: *mut UObject) -> String {
    if object.is_null() { return "<null>".to_owned(); }
    access.name_text(i32::from_ne_bytes(unsafe { (*object).name })).map(|units| display(&units)).unwrap_or_else(|_| "<invalid name>".to_owned())
}

fn exclude_package(access: CoreAccess<'_>, state: &Context<'_, '_>, index: usize) -> RuntimeResult<usize> {
    let key = format!("EXCLUDE{index}=");
    let key: Vec<Tchar> = key.bytes().map(Tchar::from).chain([0]).collect();
    let mut value = [0; 256];
    // SAFETY: the command cursor is a live NUL-terminated TCHAR string and
    // `value` is a writable, NUL-terminated 256-character output buffer.
    if !unsafe {
        crate::text_parse::parse_string(state.command, key.as_ptr(), value.as_mut_ptr(), value.len() as i32)
    } {
        return Ok(0);
    }
    let name = value.iter().copied().take_while(|unit| *unit != 0).map(|unit| unit as u32).collect::<Vec<_>>();
    if name.is_empty() {
        return Ok(0);
    }
    let class = access.native_class("Core.Package")?;
    if let Some(package) = unsafe { access.find_object_by_name(class, &name)? } {
        return Ok(package as usize);
    }
    let name_index = access.intern(&name, crate::name_owner::FindName::Add)?;
    // CreatePackage(NULL, Name) publishes a package as RF_Public.
    Ok(unsafe {
        access.construct_object(
            class, ptr::null_mut(), name_index,
            hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.public_flag, ptr::null_mut(),
        )? as usize
    })
}

struct ReferenceMap<'a> { state: *mut c_void, visitor: Hp2ObjectConsoleVisitor, archive: RefCell<*mut c_void>, access: CoreAccess<'a> }
impl crate::object_archive::ArchiveMapping for ReferenceMap<'_> {
    fn load_object(&self, _: i32) -> RuntimeResult<*mut UObject> { Err(RuntimeError::new("Reference archive cannot load objects")) }
    fn save_object(&self, _: *mut UObject) -> RuntimeResult<i32> { Err(RuntimeError::new("Reference archive cannot save objects")) }
    fn load_name(&self, _: i32) -> RuntimeResult<i32> { Err(RuntimeError::new("Reference archive cannot load names")) }
    fn save_name(&self, _: i32) -> RuntimeResult<i32> { Err(RuntimeError::new("Reference archive cannot save names")) }
    fn visit_object(&self, slot: *mut *mut UObject) -> RuntimeResult<()> {
        let object = unsafe { *slot };
        let status = unsafe { (self.visitor)(self.state, object as usize, *self.archive.borrow()) };
        if status == 0 { Ok(()) } else { Err(RuntimeError::new("Core object reference visitor failed")) }
    }
    fn visit_name(&self, _: i32) -> RuntimeResult<()> { Ok(()) }
}

struct CountingMap;
impl crate::object_archive::ArchiveMapping for CountingMap {
    fn load_object(&self, _: i32) -> RuntimeResult<*mut UObject> {
        Err(RuntimeError::new("Count archive cannot load objects"))
    }
    fn save_object(&self, object: *mut UObject) -> RuntimeResult<i32> {
        if object.is_null() { Ok(0) } else { Ok(unsafe { (*object).index }) }
    }
    fn load_name(&self, _: i32) -> RuntimeResult<i32> {
        Err(RuntimeError::new("Count archive cannot load names"))
    }
    fn save_name(&self, index: i32) -> RuntimeResult<i32> { Ok(index) }
    fn visit_object(&self, _: *mut *mut UObject) -> RuntimeResult<()> { Ok(()) }
    fn visit_name(&self, _: i32) -> RuntimeResult<()> { Ok(()) }
}

unsafe extern "C" fn archive(
    opaque: *mut c_void, source: usize, mode: i32, state: *mut c_void, visitor: Hp2ObjectConsoleVisitor,
) -> i32 {
    let context = unsafe { &mut *opaque.cast::<Context<'_, '_>>() };
    let mapping = ReferenceMap { state, visitor, archive: RefCell::new(ptr::null_mut()), access: context.access };
    let archive = crate::object_archive::ObjectArchive::reference(
        crate::object_archive::ArchiveFlags::new(0, 0, false), &mapping,
    );
    *mapping.archive.borrow_mut() = (&archive as *const _ as *mut c_void);
    let result = if mode == 1 {
        let native = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.native;
        for index in 0..context.access.object_count() {
            let object = context.access.object_at(index);
            if !object.is_null() && (unsafe { (*object).object_flags } & native != 0 || (0..context.access.root_count()).any(|i| context.access.root_at(i) == object)) {
                if let Err(error) = unsafe { context.access.serialize_object(object, &archive) } { return context.fail(error); }
            }
        }
        Ok(())
    } else {
        unsafe { context.access.serialize_object(source as *mut UObject, &archive) }
    };
    match result { Ok(()) => 0, Err(error) => context.fail(error) }
}

unsafe extern "C" fn reference_visit(state: *mut c_void, object: usize, archive: *mut c_void) -> i32 {
    match unsafe { object_console::reference(state, object, archive) } {
        Ok(()) => 0,
        Err(status) => status,
    }
}

static SERVICES: Hp2ObjectConsoleServices = Hp2ObjectConsoleServices {
    call, archive, command, log: log_event,
};

/// Runs UObject::StaticExec over canonical Core records and Rust-owned output.
///
/// # Safety
/// `command` is a live NUL-terminated TCHAR string for this synchronous call.
pub unsafe fn execute_static_command(
    access: CoreAccess<'_>, command: *const u32, output: &mut dyn FnMut(&[u32]) -> RuntimeResult<()>,
) -> RuntimeResult<bool> {
    let mut context = Context {
        access, command: command.cast(), output, class_name: [0; 256], property_name: [0; 256],
        failure: RefCell::new(None),
    };
    let context = (&mut context as *mut Context<'_, '_>).cast::<c_void>();
    // SAFETY: context, service table and input remain live through all callbacks.
    match unsafe { object_console::exec(context, &SERVICES, reference_visit) } {
        Ok(handled) => Ok(handled != 0),
        Err(status) => Err(unsafe { &*context.cast::<Context<'_, '_>>() }.failure("dispatch", status)),
    }
}
