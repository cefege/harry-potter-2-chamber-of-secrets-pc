//! Direct Rust UCC execution over the canonical Core object and VM owners.

use crate::core_access::{CoreAccess, CoreOptions, CoreOwner, CoreProcessMode, RuntimeError, RuntimeResult};
use crate::name_owner::FindName;
use hp2_abi::engine::generated::{FRegistryObjectInfo, TArrayRaw, UClass, UField, UFunction, UObject, UProperty, UStruct};
use std::cell::RefCell;
use std::ffi::{CStr, CString, OsString, c_char, c_void};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr;

const LOAD_NO_FAIL: u32 = 0x0001;
const LOAD_NO_WARN_AND_QUIET: u32 = 0x2002;
const LOAD_DISALLOW_FILES: u32 = 0x0040;
const RF_HAS_STACK: u32 = 0x0200_0000;
const PROPERTY_IMPORT_LOCALIZED: u32 = 1;

struct Runtime<O: UccOwner> {
    owner: O,
}

/// Owns the canonical object graph used for one UCC application session.
///
/// Implementations must keep the owner alive until `pre_exit` and subsequent
/// teardown of the UCC session have completed.
pub trait UccOwner {
    fn access(&self) -> CoreAccess<'_>;
    fn pre_exit(&self) -> RuntimeResult<()>;
}

impl UccOwner for CoreOwner {
    fn access(&self) -> CoreAccess<'_> { CoreOwner::access(self) }
    fn pre_exit(&self) -> RuntimeResult<()> { CoreOwner::pre_exit(self) }
}

struct UccHost<O: UccOwner, F> {
    data_root: PathBuf,
    owner_factory: RefCell<Option<F>>,
    runtime: RefCell<Option<Runtime<O>>>,
}

fn output(access: CoreAccess<'_>, category: i32, text: &[u32]) -> RuntimeResult<()> {
    let event = access.name_text(category)?;
    if equal_ascii(&event, "Title") {
        return Ok(());
    }
    if access.log_to_stdout() {
        console_log(text)?;
    }
    access.write_log_record(&event, text)
}



fn console_log(text: &[u32]) -> RuntimeResult<()> {
    let mut stdout = std::io::stdout().lock();
    for &unit in text {
        if unit == 0 { break; }
        let character = char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER);
        write!(stdout, "{character}").map_err(|error| RuntimeError::new("UCC output failed").with_context(&error.to_string()))?;
    }
    writeln!(stdout).map_err(|error| RuntimeError::new("UCC output failed").with_context(&error.to_string()))
}

fn feedback(access: CoreAccess<'_>, text: &[u32]) -> RuntimeResult<()> {
    console_log(text)?;
    let event = utf32("Log");
    access.write_log_record(&event, text)
}

fn open_log(user_root: &Path, command_line: &[u32]) -> RuntimeResult<File> {
    let source = units_to_i32(command_line);
    let log = parse_config_option(&source, "LOG=")?;
    let absolute = if log.is_none() { parse_config_option(&source, "ABSLOG=")? } else { None };
    let path = if let Some(log) = log {
        let text = units_to_path_text(&log)?.replace('\\', "/");
        let relative = text.trim_start_matches('/');
        user_root.join(relative)
    } else if let Some(absolute) = absolute {
        PathBuf::from(units_to_path_text(&absolute)?)
    } else {
        user_root.join("UCC.log")
    };
    let mut file = OpenOptions::new().create(true).truncate(true).write(true).open(&path)
        .map_err(|error| RuntimeError::new("UCC log file could not be opened")
            .with_context(&path.to_string_lossy()).with_context(&error.to_string()))?;
    file.write_all(&[0xff, 0xfe]).and_then(|_| file.flush())
        .map_err(|error| RuntimeError::new("UCC log file could not be initialized")
            .with_context(&path.to_string_lossy()).with_context(&error.to_string()))?;
    Ok(file)
}

fn units_to_path_text(text: &[u32]) -> RuntimeResult<String> {
    text.iter().copied().take_while(|&unit| unit != 0)
        .map(char::from_u32).collect::<Option<String>>()
        .ok_or_else(|| RuntimeError::new("UCC log path contains invalid Unicode"))
}

fn parse_config_option(text: &[i32], key: &str) -> RuntimeResult<Option<Vec<u32>>> {
    let key: Vec<i32> = key.bytes().map(i32::from).chain([0]).collect();
    let mut value = vec![0i32; 1024];
    // SAFETY: buffers are NUL-terminated TCHAR strings with writable output space.
    Ok(unsafe {
        crate::text_parse::parse_string(text.as_ptr(), key.as_ptr(), value.as_mut_ptr(), value.len() as i32)
    }.then(|| value.into_iter().take_while(|&unit| unit != 0).map(|unit| unit as u32).collect()))
}

impl<O, F> UccHost<O, F>
where
    O: UccOwner,
    F: FnOnce(CoreOptions) -> RuntimeResult<O>,
{
    fn with_runtime<T>(&self, body: impl FnOnce(&Runtime<O>, CoreAccess<'_>) -> RuntimeResult<T>) -> RuntimeResult<T> {
        let runtime = self.runtime.borrow();
        let runtime = runtime.as_ref().ok_or_else(|| RuntimeError::new("Core UCC is not initialized"))?;
        body(runtime, runtime.owner.access())
    }

    fn initialize(&self, command_line: &[u32]) -> RuntimeResult<()> {
        if self.runtime.borrow().is_some() {
            return Err(RuntimeError::new("Core UCC is already initialized"));
        }
        let command_text = units_to_path_text(command_line)?;
        let config_paths = hp2_bootstrap_paths::bootstrap_paths::runtime_configuration_paths(
            &self.data_root,
            command_text.as_bytes(),
        ).map_err(|error| RuntimeError::new("UCC configuration initialization failed").with_context(&error))?;
        let log_file = open_log(&config_paths.user_root, command_line)?;
        let options = CoreOptions {
            data_root: self.data_root.clone(),
            system_ini: config_paths.system_ini,
            user_ini: config_paths.user_ini,
            command_line: command_line.to_vec(),
            output,
            log_file: Some(log_file),
        };
        let factory = self.owner_factory.borrow_mut().take()
            .ok_or_else(|| RuntimeError::new("Core UCC owner factory is unavailable"))?;
        let owner = factory(options)?;
        let access = owner.access();
        access.set_process_mode(CoreProcessMode {
            is_client: true,
            is_server: true,
            is_editor: true,
            is_scriptable: true,
            lazy_load: false,
        });
        *self.runtime.borrow_mut() = Some(Runtime::<O> { owner });
        Ok(())
    }

    fn make_registry(access: CoreAccess<'_>) -> RuntimeResult<RegistryStorage> {
        let system_class = access.native_class("System")?;
        let system = unsafe { access.default_object(system_class)? }.cast::<crate::core_records::SystemRecord>();
        let discovery = DirectRegistryDiscovery { access, system, rows: RefCell::new(Vec::new()) };
        crate::object_registry::discover_public_with(&discovery)?;
        RegistryStorage::new(discovery.rows.into_inner())
    }


    fn initialize_commandlet(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
        unsafe { access.initialize_execution(object) }
    }

    fn parse_commandlet_parms(access: CoreAccess<'_>, object: *mut UObject, parms: *const u32) -> RuntimeResult<()> {
        if object.is_null() || parms.is_null() { return Err(RuntimeError::new("Invalid commandlet parameters")); }
        let class = unsafe { (*object).class.cast::<UClass>() };
        let property_class = access.native_class("Property")?;
        let object_class = access.native_class("Object")?;
        let default = unsafe { access.default_object(class)? };
        let source = units_to_i32(&unsafe { nul_terminated(parms) });
        let mut properties = Vec::new();
        let mut structure = class.cast::<UStruct>();
        while !structure.is_null() {
            let mut field = unsafe { (*structure).children.cast::<UField>() };
            while !field.is_null() {
                if unsafe { crate::object_model::is_a(field.cast(), property_class.cast()) } {
                    let owner = unsafe { (*field.cast::<UObject>()).outer.cast::<UClass>() };
                    if owner != object_class {
                        let name_index = i32::from_ne_bytes(unsafe { (*field.cast::<UObject>()).name });
                        let name = access.name_text(name_index)?;
                        properties.push((field as usize, units_to_i32(&name)));
                    }
                }
                field = unsafe { (*field).next.cast() };
            }
            structure = unsafe { (*structure.cast::<UField>()).super_field.cast::<UStruct>() };
        }
        crate::object_instance::parse_property_assignments(&source, properties, |property, text| {
            let value: Vec<u32> = text.iter().map(|&unit| unit as u32).collect();
            unsafe {
                access.property_import_text(
                    property as *mut UProperty,
                    object,
                    &value,
                    default,
                    PROPERTY_IMPORT_LOCALIZED as u32,
                )?;
            }
            Ok(())
        })
    }
}

struct DirectRegistryDiscovery<'a> {
    access: CoreAccess<'a>,
    system: *mut crate::core_records::SystemRecord,
    rows: RefCell<Vec<(Vec<u32>, Vec<u32>, Vec<u32>)>>,
}

struct DirectRegistryFiles(Vec<OsString>);
impl crate::object_registry::RegistryFiles for DirectRegistryFiles {
    type Error = RuntimeError;
    fn len(&self) -> usize { self.0.len() }
    fn name(&self, index: usize) -> RuntimeResult<Vec<i32>> {
        let text = self.0[index].to_str().ok_or_else(|| RuntimeError::new("Invalid Unicode file name"))?;
        Ok(text.chars().map(|ch| ch as i32).chain([0]).collect())
    }
    fn clear(self) -> RuntimeResult<()> { drop(self); Ok(()) }
}

impl crate::object_registry::RegistryDiscoveryOps for DirectRegistryDiscovery<'_> {
    type Error = RuntimeError;
    type Files = DirectRegistryFiles;
    fn path_count(&self) -> RuntimeResult<usize> {
        usize::try_from(unsafe { (*self.system).paths.num })
            .map_err(|_| RuntimeError::new("Invalid System.Paths count"))
    }
    fn path(&self, index: usize) -> RuntimeResult<Vec<i32>> {
        let path = unsafe { (*self.system).paths.data.cast::<TArrayRaw>().add(index).read_unaligned() };
        let count = usize::try_from(path.num).map_err(|_| RuntimeError::new("Invalid System.Paths string"))?;
        if count == 0 { return Ok(vec![0]); }
        if path.data.is_null() { return Err(RuntimeError::new("Null System.Paths string")); }
        Ok(unsafe { std::slice::from_raw_parts(path.data.cast::<i32>(), count) }.to_vec())
    }
    fn base_dir(&self) -> RuntimeResult<Vec<i32>> {
        let mut base = self.access.data_root().join("System").into_os_string();
        base.push("/");
        let text = base.to_str().ok_or_else(|| RuntimeError::new("Invalid Unicode data root"))?;
        Ok(text.chars().map(|ch| ch as i32).chain([0]).collect())
    }
    fn language(&self) -> RuntimeResult<Vec<i32>> {
        let mut language = units_to_i32(&unsafe { &*self.access.config_storage() }.language_text());
        language.push(0);
        Ok(language)
    }
    fn files(&self, pattern: &[i32]) -> RuntimeResult<DirectRegistryFiles> {
        let text = pattern.iter().copied().take_while(|&unit| unit != 0)
            .map(|unit| char::from_u32(unit as u32)).collect::<Option<String>>()
            .ok_or_else(|| RuntimeError::new("Invalid Unicode file name"))?;
        let spec = Path::new(&text);
        let directory = spec.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let pattern = CString::new(spec.file_name().unwrap_or_default().as_bytes())
            .map_err(|_| RuntimeError::new("Invalid registry file pattern"))?;
        let mut files = Vec::new();
        // Native FindFiles returns an empty result when opendir fails, and stops
        // at readdir failure. Preserve its unsorted order and lstat file filter.
        if let Ok(entries) = std::fs::read_dir(directory) {
            for entry in entries {
                let Ok(entry) = entry else { break; };
                let name = entry.file_name();
                if name.to_str().is_none() { continue; }
                let candidate = CString::new(name.as_bytes())
                    .map_err(|_| RuntimeError::new("Invalid registry file name"))?;
                if unsafe { libc::fnmatch(pattern.as_ptr(), candidate.as_ptr(), 0) } != 0 { continue; }
                let Ok(metadata) = std::fs::symlink_metadata(entry.path()) else { continue; };
                if !metadata.is_dir() { files.push(name); }
            }
        }
        Ok(DirectRegistryFiles(files))
    }
    fn visit_public(&self, filename: &[i32]) -> RuntimeResult<bool> {
        let filename: Vec<u32> = filename.iter().copied().take_while(|&unit| unit != 0).map(|unit| unit as u32).collect();
        let section = utf32("Public");
        let properties = {
            let config = unsafe { &mut *self.access.config_storage() };
            if !config.has_section(Some(&filename), &section)? { return Ok(false); }
            config.section_pairs(Some(&filename), &section)?
        };
        for (key, value) in properties {
            if !equal_ascii(&key, "Object") { continue; }
            let mut source = units_to_i32(&value);
            source.push(0);
            let row = parse_registry_row(&mut source)?;
            if equal_ascii(&row.1, "Class") && equal_ascii(&row.2, "Core.Commandlet") {
                self.rows.borrow_mut().push(row);
            }
        }
        Ok(true)
    }
}

struct RegistryStorage {
    _entries: Vec<FRegistryObjectInfo>,
    _strings: Vec<Box<[i32]>>,
    raw: TArrayRaw,
}

impl RegistryStorage {
    fn new(rows: Vec<(Vec<u32>, Vec<u32>, Vec<u32>)>) -> RuntimeResult<Self> {
        let mut strings = Vec::with_capacity(rows.len() * 3);
        for (object, class, meta) in rows {
            for text in [object, class, meta] {
                strings.push(to_tchar_box(&text)?);
            }
        }
        let mut entries = Vec::with_capacity(strings.len() / 3);
        for row in strings.chunks_exact(3) {
            entries.push(FRegistryObjectInfo {
                object: fstring_array(row[0].as_ptr().cast_mut().cast(), row[0].len()),
                class: fstring_array(row[1].as_ptr().cast_mut().cast(), row[1].len()),
                meta_class: fstring_array(row[2].as_ptr().cast_mut().cast(), row[2].len()),
                description: empty_array(),
                autodetect: empty_array(),
            });
        }
        let raw = TArrayRaw {
            data: if entries.is_empty() { ptr::null_mut() } else { entries.as_mut_ptr().cast() },
            num: entries.len() as i32,
            max: entries.len() as i32,
        };
Ok(Self { _entries: entries, _strings: strings, raw })
    }
}

fn fstring_array(data: *mut c_void, len: usize) -> TArrayRaw {
    TArrayRaw { data, num: len as i32, max: len as i32 }
}
fn empty_array() -> TArrayRaw { fstring_array(ptr::null_mut(), 0) }
fn to_tchar_box(text: &[u32]) -> RuntimeResult<Box<[i32]>> {
    let mut values = Vec::with_capacity(text.len() + 1);
    values.extend(text.iter().copied().take_while(|unit| *unit != 0).map(|unit| i32::try_from(unit).unwrap_or(i32::MAX)));
    values.push(0);
    Ok(values.into_boxed_slice())
}
fn units_to_i32(text: &[u32]) -> Vec<i32> {
    text.iter().copied().map(|unit| i32::try_from(unit).unwrap_or(i32::MAX)).collect()
}
fn parse_registry_row(source: &mut [i32]) -> RuntimeResult<(Vec<u32>, Vec<u32>, Vec<u32>)> {
    let offset = crate::object_registry::prepare_registry_value(source);
    let value = &source[offset..];
    Ok((
        parse_config_value(value, "Name=")?,
        parse_config_value(value, "Class=")?,
        parse_config_value(value, "MetaClass=")?,
    ))
}
fn parse_config_value(text: &[i32], key: &str) -> RuntimeResult<Vec<u32>> {
    let key: Vec<i32> = key.bytes().map(i32::from).chain([0]).collect();
    let mut value = vec![0i32; 4096];
    // SAFETY: all buffers are NUL-terminated TCHAR data and value has positive capacity.
    if !unsafe { crate::text_parse::parse_string(text.as_ptr(), key.as_ptr(), value.as_mut_ptr(), value.len() as i32) } {
        return Ok(Vec::new());
    }
    Ok(value.into_iter().take_while(|&unit| unit != 0).map(|unit| unit as u32).collect())
}
fn equal_ascii(value: &[u32], text: &str) -> bool {
    let mut actual = value.iter().copied().take_while(|&unit| unit != 0);
    let mut expected = text.bytes();
    loop {
        match (actual.next(), expected.next()) {
            (Some(left), Some(right)) if char::from_u32(left).is_some_and(|ch| ch.eq_ignore_ascii_case(&(right as char))) => {}
            (None, None) => return true,
            _ => return false,
        }
    }
}
fn utf32(text: &str) -> Vec<u32> { text.chars().map(u32::from).chain([0]).collect() }
unsafe fn nul_terminated(mut text: *const u32) -> Vec<u32> {
    let mut result = Vec::new();
    if text.is_null() { return result; }
    loop {
        let unit = unsafe { *text };
        result.push(unit);
        if unit == 0 { return result; }
        text = unsafe { text.add(1) };
    }
}

impl<O, F> crate::ucc::UccHost for UccHost<O, F>
where
    O: UccOwner,
    F: FnOnce(CoreOptions) -> RuntimeResult<O>,
{
    type Error = RuntimeError;

    fn app_init(&self, command_line: &[u32]) -> RuntimeResult<()> { self.initialize(command_line) }

    fn set_language(&self, language: &[u32]) -> RuntimeResult<()> {
        self.with_runtime(|_, access| access.set_language(language))
    }

    fn with_registry<T>(&self, body: impl FnOnce(*const TArrayRaw) -> RuntimeResult<T>) -> RuntimeResult<T> {
        let registry = self.with_runtime(|_, access| Self::make_registry(access))?;
        body(&registry.raw)
    }

    fn check_failed(&self, expr: &CStr, line: i32) -> RuntimeResult<()> {
        Err(RuntimeError::new("UCC verification failed")
            .with_context(&format!("{} at line {line}", expr.to_string_lossy())))
    }

    fn commandlet_class(&self) -> RuntimeResult<*mut c_void> {
        self.with_runtime(|_, access| access.native_class("Commandlet").map(|class| class.cast()))
    }

    fn load_commandlet_class(&self, name: &[u32], flags: u32) -> RuntimeResult<*mut c_void> {
        self.with_runtime(|_, access| {
            let base = access.native_class("Commandlet")?;
            let name = terminated(name);
            unsafe { access.static_load_class(base, ptr::null_mut(), &name, flags) }.map(|class| class.cast())
        })
    }

    fn command_line(&self) -> RuntimeResult<*const u32> {
        self.with_runtime(|_, access| Ok(access.command_line().as_ptr()))
    }

    fn default_object(&self, class: *mut c_void) -> RuntimeResult<*mut c_void> {
        self.with_runtime(|_, access| unsafe { access.default_object(class.cast()) }.map(|object| object.cast()))
    }

    fn with_class_full_name<T>(&self, class: *mut c_void, body: impl FnOnce(*const u32) -> RuntimeResult<T>) -> RuntimeResult<T> {
        let mut text = self.with_runtime(|_, access| crate::core_lifetime::object_full_name(access, class.cast()))?;
        text.push(0);
        body(text.as_ptr())
    }

    fn construct_commandlet(&self, class: *mut c_void) -> RuntimeResult<*mut c_void> {
        self.with_runtime(|_, access| unsafe {
            access.construct_object(class.cast(), access.transient_package(), 0, 0x0000_4000, ptr::null_mut())
        }.map(|object| object.cast()))
    }

    fn init_execution(&self, commandlet: *mut c_void) -> RuntimeResult<()> {
        self.with_runtime(|_, access| Self::initialize_commandlet(access, commandlet.cast()))
    }

    fn parse_parms(&self, commandlet: *mut c_void, parms: *const u32) -> RuntimeResult<()> {
        self.with_runtime(|_, access| Self::parse_commandlet_parms(access, commandlet.cast(), parms))
    }

    fn main(&self, commandlet: *mut c_void, parms: *const u32) -> RuntimeResult<i32> {
        self.with_runtime(|_, access| unsafe { access.commandlet_main(commandlet.cast(), parms) })
    }

    fn warn_log(&self, text: &[u32]) -> RuntimeResult<()> {
        self.with_runtime(|_, access| feedback(access, text))
    }
    fn warn_logf(&self, text: &[u32]) -> RuntimeResult<()> {
        self.with_runtime(|_, access| feedback(access, text))
    }
    fn gwarn_logf(&self, text: &[u32]) -> RuntimeResult<()> {
        self.with_runtime(|_, access| feedback(access, text))
    }
    fn debug_log(&self, text: &[u32]) -> RuntimeResult<()> {
        self.with_runtime(|_, access| {
            let id = access.intern_str("Log", FindName::Add)?;
            access.emit_log(id, text)
        })
    }



    fn pre_exit(&self) -> RuntimeResult<()> {
        self.with_runtime(|runtime, _| runtime.owner.pre_exit())
    }

    fn set_startup_mode(&self) -> RuntimeResult<()> {
        self.with_runtime(|_, access| {
            access.set_process_mode(CoreProcessMode { is_client: true, is_server: true, is_editor: true, is_scriptable: true, lazy_load: false });
            Ok(())
        })
    }

    fn set_editor(&self, editor: bool) -> RuntimeResult<()> {
        self.with_runtime(|_, access| {
            let mut mode = access.process_mode();
            mode.is_editor = editor;
            access.set_process_mode(mode);
            Ok(())
        })
    }

    fn set_commandlet_mode(&self, client: bool, server: bool, editor: bool, lazy: bool) -> RuntimeResult<()> {
        self.with_runtime(|_, access| {
            let mut mode = access.process_mode();
            mode.is_client = client;
            mode.is_server = server;
            mode.is_editor = editor;
            mode.lazy_load = lazy;
            access.set_process_mode(mode);
            Ok(())
        })
    }

    fn set_log_to_stdout(&self, enabled: bool) -> RuntimeResult<()> {
        self.with_runtime(|_, access| {
            access.set_log_to_stdout(enabled);
            Ok(())
        })
    }

    fn warning_count(&self) -> i32 {
        self.runtime.borrow().as_ref().map_or(0, |runtime| runtime.owner.access().warning_count())
    }
    fn load_quiet(&self) -> u32 { LOAD_NO_WARN_AND_QUIET }
    fn load_disallow_files(&self) -> u32 { LOAD_DISALLOW_FILES }
    fn load_no_fail(&self) -> u32 { LOAD_NO_FAIL }
}



fn terminated(text: &[u32]) -> Vec<u32> {
    let mut result: Vec<u32> = text.iter().copied().take_while(|&unit| unit != 0).collect();
    result.push(0);
    result
}

fn paths(args: &[CString]) -> RuntimeResult<PathBuf> {
    let explicit = args.iter().skip(1).find_map(|arg| {
        let bytes = arg.as_bytes();
        hp2_bootstrap_paths::bootstrap_paths::argument(bytes).map(|offset| &bytes[offset..])
    });
    let root = if let Some(explicit) = explicit {
        let result = hp2_bootstrap_paths::bootstrap_paths::validate(explicit);
        if !result.success {
            return Err(RuntimeError::new("Invalid -datadir: expected valid game data")
                .with_context(&String::from_utf8_lossy(explicit)));
        }
        PathBuf::from(std::ffi::OsStr::from_bytes(&result.text[0]))
    } else {
        let result = hp2_bootstrap_paths::bootstrap_paths::run(8, b"", b"", &[], |_, _| 0);
        if !result.success { return Err(RuntimeError::new("No game data installed")); }
        PathBuf::from(std::ffi::OsStr::from_bytes(&result.text[0]))
    };
    Ok(root)
}

/// Runs the Rust-owned UCC session with process arguments including argv[0].
pub fn run(args: impl IntoIterator<Item = OsString>) -> i32 {
    run_with_owner(args, CoreOwner::new)
}

/// Runs UCC using an owner factory while retaining the shared UCC bootstrap,
/// commandlet dispatch, and teardown policy.
pub fn run_with_owner<O, F>(args: impl IntoIterator<Item = OsString>, owner_factory: F) -> i32
where
    O: UccOwner,
    F: FnOnce(CoreOptions) -> RuntimeResult<O>,
{
    let args = args.into_iter().map(|arg| CString::new(arg.as_os_str().as_bytes())).collect::<Result<Vec<_>, _>>();
    let args = match args {
        Ok(args) => args,
        Err(_) => { let _ = std::io::stderr().write_all(b"ucc: argument contains a NUL byte\n"); return 1; }
    };
    let data_root = match paths(&args) {
        Ok(path) => path,
        Err(error) => { let _ = writeln!(std::io::stderr(), "{error}"); return 1; }
    };
    let host = UccHost { data_root, owner_factory: RefCell::new(Some(owner_factory)), runtime: RefCell::new(None) };
    let argv: Vec<*const c_char> = args.iter().map(|arg| arg.as_ptr()).collect();
    // SAFETY: CString owns every NUL-terminated argument until the session returns.
    let result = unsafe {
        crate::ucc::run_session(&host, argv.len() as i32, argv.as_ptr(), |arg| {
            if arg.is_null() { return false; }
            let bytes = unsafe { CStr::from_ptr(arg) }.to_bytes();
            hp2_bootstrap_paths::bootstrap_paths::argument(bytes).is_some()
        })
    };
    match result {
        Ok(Some(status)) => status,
        Ok(None) => 0,
        Err(error) => { let _ = writeln!(std::io::stdout(), "{error}\n\nExiting due to error"); 1 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_tuple_delimiters_are_not_part_of_class_identity() {
        let mut source = "(Name=Core.HelloWorldCommandlet,Class=Class,MetaClass=Core.Commandlet)".encode_utf16().map(i32::from).collect::<Vec<_>>();
        source.push(0);
        let (object, class, meta) = parse_registry_row(&mut source).expect("parse registry row");
        assert_eq!(object, utf32("Core.HelloWorldCommandlet")[..25]);
        assert_eq!(class, utf32("Class")[..5]);
        assert_eq!(meta, utf32("Core.Commandlet")[..15]);
    }
}
