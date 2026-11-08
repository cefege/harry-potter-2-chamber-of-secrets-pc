//! Shared Rust policy for the `ucc` command-line launcher.
//!
//! The ordinary Core host and temporary mixed-runtime adapter execute the same
//! argument, help, command selection, and commandlet sequencing algorithm.

use core::{cell::Cell, convert::Infallible, ffi::c_void, mem, ptr};
use std::{ffi::c_char, io::Write};

use hp2_abi::engine::generated::{FRegistryObjectInfo, TArrayRaw, UCommandlet};
use crate::{ansi, sort::quicksort, tokenize::parse_token};
use crate::lit;

use crate::native_text::{
    TextPart as A, fmt, fstring_is_empty, fstring_text, fstring_units, text_len,
};

/// `TCHAR Command[1024]`, `CmdLine[1024]`, `Argument[1024]`, and the buffer of
/// the `FString` `ParseToken` overload.
const TEXT_CAPACITY: usize = 1024;
/// `RightPad( Default->HelpCmd, 21 )` in the listing.
const COMMAND_PAD: usize = 21;
/// `RightPad( Default->HelpParm[i], 16 )` in per-commandlet help.
const PARAMETER_PAD: usize = 16;
/// `UCC.cpp` line of the original `verify` on `Core.Commandlet`.
const VERIFY_LINE: i32 = 167;
/// `#expr` of that `verify`.
const VERIFY_EXPR: &core::ffi::CStr = c"UObject::StaticLoadClass( UCommandlet::StaticClass(), NULL, TEXT(\"Core.Commandlet\"), NULL, LOAD_NoFail, NULL )==UCommandlet::StaticClass()";

/// Continuation a scope leaf invokes with the address of its stack object.
pub type Hp2UccContinuation = unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32;

/// Process state the shell owns. Pointers address the C++ globals; the
/// output devices are already converted to `FOutputDevice*`.
#[repr(C)]
pub struct Hp2UccProcess {
    /// `&GIsClient`.
    pub is_client: *mut i32,
    /// `&GIsServer`.
    pub is_server: *mut i32,
    /// `&GIsEditor`.
    pub is_editor: *mut i32,
    /// `&GIsScriptable`.
    pub is_scriptable: *mut i32,
    /// `&GLazyLoad`.
    pub lazy_load: *mut i32,
    /// `&GLog`.
    pub log: *mut *mut c_void,
    /// `&Warn.AuxOut`.
    pub warn_aux_out: *mut *mut c_void,
    /// `&Warn` as `FOutputDevice*`.
    pub warn: *mut c_void,
    /// `&Log` as `FOutputDevice*`.
    pub file_log: *mut c_void,
    /// `&Warn.WarningCount`.
    pub warn_warning_count: *const i32,
    /// `LOAD_NoWarn | LOAD_Quiet`.
    pub load_quiet: u32,
    /// `LOAD_DisallowFiles`.
    pub load_disallow_files: u32,
    /// `LOAD_NoFail`.
    pub load_no_fail: u32,
}

/// Native leaves for the `ucc` session. Text is `TCHAR` (`u32`); `expr` is ANSI.
/// Each leaf returns 0, or nonzero after capturing a C++ exception.
#[repr(C)]
pub struct Hp2UccServices {
    /// `appInit( TEXT("HP"), cmd_line, ... )` with the shell's devices, then
    /// `RegisterHP2RuntimeClasses()` in static-link builds.
    pub app_init: unsafe extern "C" fn(cmd_line: *const u32) -> i32,
    /// `UObject::SetLanguage(language)`.
    pub set_language: unsafe extern "C" fn(language: *const u32) -> i32,
    /// `TArray<FRegistryObjectInfo> List; UObject::GetRegistryObjects( List,
    /// UClass::StaticClass(), UCommandlet::StaticClass(), 0 )`; resumes with
    /// `&List`, destroyed when the scope ends.
    pub registry_scope:
        unsafe extern "C" fn(context: *mut c_void, resume: Hp2UccContinuation) -> i32,
    /// `appFailAssert(expr, __FILE__, line)` in the shell's translation unit.
    pub check_failed: unsafe extern "C" fn(expr: *const c_char, line: i32) -> i32,
    /// `UCommandlet::StaticClass()`.
    pub commandlet_class: unsafe extern "C" fn(result: *mut *mut c_void) -> i32,
    /// `UObject::StaticLoadClass( UCommandlet::StaticClass(), NULL, name, NULL, flags, NULL )`.
    pub load_commandlet_class:
        unsafe extern "C" fn(name: *const u32, flags: u32, result: *mut *mut c_void) -> i32,
    /// `appCmdLine()`.
    pub app_cmd_line: unsafe extern "C" fn(result: *mut *const u32) -> i32,
    /// `Class->GetDefaultObject()`.
    pub default_object: unsafe extern "C" fn(uclass: *mut c_void, result: *mut *mut c_void) -> i32,
    /// `Class->GetFullName()`.
    pub class_full_name: unsafe extern "C" fn(uclass: *mut c_void, result: *mut *const u32) -> i32,
    /// `ConstructObject<UCommandlet>(Class)`.
    pub construct_commandlet:
        unsafe extern "C" fn(uclass: *mut c_void, result: *mut *mut c_void) -> i32,
    /// `Commandlet->InitExecution()`.
    pub init_execution: unsafe extern "C" fn(commandlet: *mut c_void) -> i32,
    /// `Commandlet->ParseParms(parms)`.
    pub parse_parms: unsafe extern "C" fn(commandlet: *mut c_void, parms: *const u32) -> i32,
    /// `Commandlet->Main(parms)`.
    pub main:
        unsafe extern "C" fn(commandlet: *mut c_void, parms: *const u32, result: *mut i32) -> i32,
    /// `Warn.Log(text)`.
    pub warn_log: unsafe extern "C" fn(text: *const u32) -> i32,
    /// `Warn.Logf( TEXT("%s"), text )`.
    pub warn_logf: unsafe extern "C" fn(text: *const u32) -> i32,
    /// `GWarn->Logf( TEXT("%s"), text )`.
    pub gwarn_logf: unsafe extern "C" fn(text: *const u32) -> i32,
    /// `debugf( TEXT("%s"), text )`.
    pub debug_log: unsafe extern "C" fn(text: *const u32) -> i32,
    /// `appPreExit(); GIsGuarded = 0;`.
    pub pre_exit: unsafe extern "C" fn() -> i32,
}

macro_rules! value {
    ($callback:expr $(, $arg:expr)* $(,)?) => {{
        let mut out = mem::MaybeUninit::uninit();
        if ($callback)($($arg,)* out.as_mut_ptr()) != 0 { return Err(()); }
        out.assume_init()
    }};
}
macro_rules! action {
    ($callback:expr $(, $arg:expr)* $(,)?) => {{
        if ($callback)($($arg),*) != 0 { return Err(()); }
    }};
}

type R<T> = core::result::Result<T, ()>;

/// Direct Rust operations required by the shared UCC session policy.
///
/// All pointer methods operate on live canonical Core-owned objects. Returned text
/// pointers and command-line pointers remain valid for the duration of this session.
pub(crate) trait UccHost {
    type Error;

    fn app_init(&self, command_line: &[u32]) -> Result<(), Self::Error>;
    fn set_language(&self, language: &[u32]) -> Result<(), Self::Error>;
    fn with_registry<T>(
        &self,
        body: impl FnOnce(*const TArrayRaw) -> Result<T, Self::Error>,
    ) -> Result<T, Self::Error>;
    fn check_failed(&self, expr: &core::ffi::CStr, line: i32) -> Result<(), Self::Error>;
    fn commandlet_class(&self) -> Result<*mut c_void, Self::Error>;
    fn load_commandlet_class(&self, name: &[u32], flags: u32)
        -> Result<*mut c_void, Self::Error>;
    fn command_line(&self) -> Result<*const u32, Self::Error>;
    fn default_object(&self, class: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn with_class_full_name<T>(
        &self,
        class: *mut c_void,
        body: impl FnOnce(*const u32) -> Result<T, Self::Error>,
    ) -> Result<T, Self::Error>;
    fn construct_commandlet(&self, class: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn init_execution(&self, commandlet: *mut c_void) -> Result<(), Self::Error>;
    fn parse_parms(&self, commandlet: *mut c_void, parms: *const u32)
        -> Result<(), Self::Error>;
    fn main(&self, commandlet: *mut c_void, parms: *const u32) -> Result<i32, Self::Error>;
    fn warn_log(&self, text: &[u32]) -> Result<(), Self::Error>;
    fn warn_logf(&self, text: &[u32]) -> Result<(), Self::Error>;
    fn gwarn_logf(&self, text: &[u32]) -> Result<(), Self::Error>;
    fn debug_log(&self, text: &[u32]) -> Result<(), Self::Error>;
    fn pre_exit(&self) -> Result<(), Self::Error>;
    fn set_startup_mode(&self) -> Result<(), Self::Error>;
    fn set_editor(&self, editor: bool) -> Result<(), Self::Error>;
    fn set_commandlet_mode(
        &self,
        client: bool,
        server: bool,
        editor: bool,
        lazy: bool,
    ) -> Result<(), Self::Error>;
    fn set_log_to_stdout(&self, enabled: bool) -> Result<(), Self::Error>;
    fn warning_count(&self) -> i32;
    fn load_quiet(&self) -> u32;
    fn load_disallow_files(&self) -> u32;
    fn load_no_fail(&self) -> u32;
}

/// Runs the shared command-line and commandlet policy over a Rust-owned Core host.
///
/// # Safety
/// `argv` must contain `argc` live, NUL-terminated C strings.
pub(crate) unsafe fn run_session<H: UccHost>(
    host: &H,
    argc: i32,
    argv: *const *const c_char,
    is_data_directory_argument: impl Fn(*const c_char) -> bool,
) -> Result<Option<i32>, H::Error> {
    let (command, cmd_line) = match unsafe {
        prepare_arguments(argc, argv, &is_data_directory_argument)
    } {
        Ok(arguments) => arguments,
        Err(status) => return Ok(Some(status)),
    };
    host.app_init(&cmd_line)?;
    host.set_language(lit!("int"))?;
    host.with_registry(|list| run_dispatch(host, list, command))
        .map(|()| None)
}

unsafe fn prepare_arguments(
    argc: i32,
    argv: *const *const c_char,
    is_data_directory_argument: &impl Fn(*const c_char) -> bool,
) -> Result<(Vec<u32>, Vec<u32>), i32> {
    let arg = |index: i32| unsafe { *argv.add(index as usize) };
    let mut command_index = 1;
    while command_index < argc && is_data_directory_argument(arg(command_index)) { command_index += 1; }
    let mut command = [0u32; TEXT_CAPACITY];
    if command_index < argc && !unsafe { from_utf8(&mut command, arg(command_index)) } {
        return Err(reject(b"ucc: command is not valid UTF-8 or is too long\n"));
    }
    let mut cmd_line = [0u32; TEXT_CAPACITY];
    let mut cmd_line_len = 0;
    for index in command_index + 1..argc {
        if is_data_directory_argument(arg(index)) { continue; }
        let mut argument = [0u32; TEXT_CAPACITY];
        if !unsafe { from_utf8(&mut argument, arg(index)) } {
            return Err(reject(b"ucc: parameter is not valid UTF-8 or is too long\n"));
        }
        let argument_len = unsafe { text_len(argument.as_ptr()) };
        let separator_len = usize::from(cmd_line_len != 0);
        if cmd_line_len + separator_len + argument_len >= TEXT_CAPACITY {
            return Err(reject(b"ucc: command line is too long\n"));
        }
        if separator_len != 0 { cmd_line[cmd_line_len] = b' ' as u32; cmd_line_len += 1; }
        cmd_line[cmd_line_len..cmd_line_len + argument_len].copy_from_slice(&argument[..argument_len]);
        cmd_line_len += argument_len;
    }
    Ok((nul_terminated(&command), nul_terminated(&cmd_line[..cmd_line_len])))
}

/// Runs the temporary mixed-runtime ABI adapter over the same policy kernel.
///
/// # Safety
/// `argv` must hold `argc` live C strings; `s` and `p` must be the shell's.
pub unsafe fn session(
    s: &Hp2UccServices,
    p: &Hp2UccProcess,
    argc: i32,
    argv: *const *const c_char,
    is_data_directory_argument: impl Fn(*const c_char) -> bool,
) -> R<Option<i32>> {
    let host = LegacyHost { services: s, process: p };
    unsafe { run_session(&host, argc, argv, is_data_directory_argument) }
}

#[derive(Clone, Copy)]
struct LegacyHost<'a> {
    services: &'a Hp2UccServices,
    process: &'a Hp2UccProcess,
}
impl UccHost for LegacyHost<'_> {
    type Error = ();

    fn app_init(&self, command_line: &[u32]) -> R<()> {
        unsafe { action!(self.services.app_init, command_line.as_ptr()) };
        Ok(())
    }
    fn set_language(&self, language: &[u32]) -> R<()> {
        unsafe { action!(self.services.set_language, language.as_ptr()) };
        Ok(())
    }
    fn with_registry<T>(&self, body: impl FnOnce(*const TArrayRaw) -> R<T>) -> R<T> {
        struct Context<T, F> { body: Option<F>, result: Option<R<T>> }
        unsafe extern "C" fn resume<T, F>(context: *mut c_void, list: *mut c_void) -> i32
        where F: FnOnce(*const TArrayRaw) -> R<T> {
            let context = unsafe { &mut *context.cast::<Context<T, F>>() };
            let Some(body) = context.body.take() else {
                context.result = Some(Err(()));
                return 1;
            };
            context.result = Some(body(list.cast()));
            i32::from(context.result.as_ref().is_some_and(Result::is_err))
        }
        fn invoke<T, F>(services: &Hp2UccServices, body: F) -> R<T>
        where F: FnOnce(*const TArrayRaw) -> R<T> {
            let mut context = Context { body: Some(body), result: None };
            unsafe {
                action!(services.registry_scope,
                    (&mut context as *mut Context<T, F>).cast(), resume::<T, F>);
            }
            context.result.unwrap_or(Err(()))
        }
        invoke(self.services, body)
    }
    fn check_failed(&self, expr: &core::ffi::CStr, line: i32) -> R<()> {
        unsafe { action!(self.services.check_failed, expr.as_ptr(), line) };
        Ok(())
    }
    fn commandlet_class(&self) -> R<*mut c_void> {
        unsafe { Ok(value!(self.services.commandlet_class)) }
    }
    fn load_commandlet_class(&self, name: &[u32], flags: u32) -> R<*mut c_void> {
        unsafe { Ok(value!(self.services.load_commandlet_class, name.as_ptr(), flags)) }
    }
    fn command_line(&self) -> R<*const u32> {
        unsafe { Ok(value!(self.services.app_cmd_line)) }
    }
    fn default_object(&self, class: *mut c_void) -> R<*mut c_void> {
        unsafe { Ok(value!(self.services.default_object, class)) }
    }
    fn with_class_full_name<T>(&self, class: *mut c_void,
        body: impl FnOnce(*const u32) -> R<T>) -> R<T> {
        unsafe { body(value!(self.services.class_full_name, class)) }
    }
    fn construct_commandlet(&self, class: *mut c_void) -> R<*mut c_void> {
        unsafe { Ok(value!(self.services.construct_commandlet, class)) }
    }
    fn init_execution(&self, commandlet: *mut c_void) -> R<()> {
        unsafe { action!(self.services.init_execution, commandlet) }; Ok(())
    }
    fn parse_parms(&self, commandlet: *mut c_void, parms: *const u32) -> R<()> {
        unsafe { action!(self.services.parse_parms, commandlet, parms) }; Ok(())
    }
    fn main(&self, commandlet: *mut c_void, parms: *const u32) -> R<i32> {
        unsafe { Ok(value!(self.services.main, commandlet, parms)) }
    }
    fn warn_log(&self, text: &[u32]) -> R<()> {
        unsafe { action!(self.services.warn_log, text.as_ptr()) }; Ok(())
    }
    fn warn_logf(&self, text: &[u32]) -> R<()> {
        unsafe { action!(self.services.warn_logf, text.as_ptr()) }; Ok(())
    }
    fn gwarn_logf(&self, text: &[u32]) -> R<()> {
        unsafe { action!(self.services.gwarn_logf, text.as_ptr()) }; Ok(())
    }
    fn debug_log(&self, text: &[u32]) -> R<()> {
        unsafe { action!(self.services.debug_log, text.as_ptr()) }; Ok(())
    }
    fn pre_exit(&self) -> R<()> {
        unsafe { action!(self.services.pre_exit) }; Ok(())
    }
    fn set_startup_mode(&self) -> R<()> {
        unsafe {
            for field in [self.process.is_client, self.process.is_server,
                self.process.is_editor, self.process.is_scriptable] {
                ptr::write(field, 1);
            }
            ptr::write(self.process.lazy_load, 0);
        }
        Ok(())
    }
    fn set_editor(&self, editor: bool) -> R<()> {
        unsafe { ptr::write(self.process.is_editor, i32::from(editor)) }; Ok(())
    }
    fn set_commandlet_mode(&self, client: bool, server: bool, editor: bool, lazy: bool) -> R<()> {
        unsafe {
            ptr::write(self.process.is_client, i32::from(client));
            ptr::write(self.process.is_server, i32::from(server));
            ptr::write(self.process.is_editor, i32::from(editor));
            ptr::write(self.process.lazy_load, i32::from(lazy));
        }
        Ok(())
    }
    fn set_log_to_stdout(&self, enabled: bool) -> R<()> {
        unsafe {
            if enabled {
                ptr::write(self.process.warn_aux_out, ptr::read(self.process.log));
                ptr::write(self.process.log, self.process.warn);
            } else {
                ptr::write(self.process.warn_aux_out, ptr::null_mut());
                ptr::write(self.process.log, self.process.file_log);
            }
        }
        Ok(())
    }
    fn warning_count(&self) -> i32 { unsafe { ptr::read(self.process.warn_warning_count) } }
    fn load_quiet(&self) -> u32 { self.process.load_quiet }
    fn load_disallow_files(&self) -> u32 { self.process.load_disallow_files }
    fn load_no_fail(&self) -> u32 { self.process.load_no_fail }
}

/// `appFromUtf8InPlace( Dest, Src, ARRAY_COUNT(Dest) )`.
unsafe fn from_utf8(dest: &mut [u32; TEXT_CAPACITY], src: *const c_char) -> bool {
    unsafe {
        ansi::from_utf8_in_place(dest.as_mut_ptr().cast(), src.cast(), TEXT_CAPACITY as i32) != 0
    }
}

/// `fprintf( stderr, message ); return 1;`.
fn reject(message: &[u8]) -> i32 {
    let _ = std::io::stderr().write_all(message);
    1
}

/// The text of `buffer` up to its NUL, NUL-terminated (`FString Token = Buffer`).
fn nul_terminated(buffer: &[u32]) -> Vec<u32> {
    let len = buffer
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(buffer.len());
    let mut text = Vec::with_capacity(len + 1);
    text.extend_from_slice(&buffer[..len]);
    text.push(0);
    text
}


fn run_dispatch<H: UccHost>(host: &H, list: *const TArrayRaw, mut token: Vec<u32>) -> Result<(), H::Error> {
    host.set_startup_mode()?;
    let mut help = false;
    let mut flags = host.load_quiet();
    let process = if token[0] == 0 {
        banner(host)?;
        host.warn_logf(lit!("Use \"ucc help\" for help"))?;
        false
    } else if stricmp(token.as_ptr(), lit!("HELP").as_ptr()) == 0 {
        banner(host)?;
        if host.load_commandlet_class(lit!("Core.Commandlet"), host.load_no_fail())? != host.commandlet_class()? {
            host.check_failed(VERIFY_EXPR, VERIFY_LINE)?;
        }
        let mut cursor = host.command_line()?;
        host.set_editor(false)?;
        let mut buffer = [0u32; TEXT_CAPACITY];
        if unsafe { parse_token((&raw mut cursor).cast(), buffer.as_mut_ptr().cast(), TEXT_CAPACITY as i32, false) } {
            token = nul_terminated(&buffer);
            help = true;
            true
        } else {
            list_commandlets(host, list, flags)?;
            false
        }
    } else {
        if stricmp(token.as_ptr(), lit!("Make").as_ptr()) == 0 { flags |= host.load_disallow_files(); }
        true
    };
    if process { run_command(host, list, &token, flags, help)?; }
    host.pre_exit()
}

fn banner<H: UccHost>(host: &H) -> Result<(), H::Error> {
    for line in [
        lit!("=======================================").as_slice(),
        lit!("ucc.exe: UnrealOS execution environment"),
        lit!("Copyright 1999 Epic Games Inc"),
        lit!("======================================="),
        lit!(""),
    ] { host.warn_logf(line)?; }
    Ok(())
}

unsafe fn right_pad(string: *const TArrayRaw, count: usize) -> Vec<u32> {
    let mut text: Vec<u32> = unsafe { fstring_units(string) }.iter().map(|&unit| unit as u32).collect();
    if text.len() < count { text.resize(count, b' ' as u32); }
    nul_terminated(&text)
}
unsafe fn registry_len(list: *const TArrayRaw) -> i32 { unsafe { ptr::read_unaligned(list).num } }
/// Copies the reflected FString name into the host API's NUL-terminated UTF-32 form.
///
/// The copy is required because FString length, not a trailing NUL, bounds its storage.
unsafe fn registry_name(list: *const TArrayRaw, index: i32) -> Vec<u32> {
    unsafe {
        let entries = ptr::read_unaligned(list).data.cast::<FRegistryObjectInfo>();
        let name = fstring_units(ptr::addr_of!((*entries.add(index as usize)).object));
        let mut terminated = Vec::with_capacity(name.len() + 1);
        terminated.extend(name.iter().map(|&unit| unit as u32));
        terminated.push(0);
        terminated
    }
}
unsafe fn short_name(mut text: *const u32) -> *const u32 {
    let mut cursor = text;
    unsafe { while *cursor != 0 { if *cursor == b'.' as u32 { text = cursor.add(1); } cursor = cursor.add(1); } }
    text
}
fn stricmp(a: *const u32, b: *const u32) -> i32 { unsafe { ansi::stricmp(a.cast(), b.cast()) } }

fn list_commandlets<H: UccHost>(host: &H, list: *const TArrayRaw, flags: u32) -> Result<(), H::Error> {
    for line in [lit!("Usage:").as_slice(), lit!("   ucc <command> <parameters>"), lit!(""), lit!("Commands for \"ucc\":")] {
        host.warn_logf(line)?;
    }
    let mut items = Vec::new();
    let mut index = 0;
    while index < unsafe { registry_len(list) } {
        let class_name = unsafe { registry_name(list, index) };
        let class = host.load_commandlet_class(&class_name, flags)?;
        if !class.is_null() {
            let default = host.default_object(class)?.cast::<UCommandlet>();
            let command = unsafe { right_pad(ptr::addr_of!((*default).help_cmd), COMMAND_PAD) };
            items.push(unsafe { fmt(&[A::L("   ucc "), A::P(command.as_ptr()), A::L(" "),
                A::P(fstring_text(ptr::addr_of!((*default).help_one_liner)))]) });
        }
        index += 1;
    }
    items.push(lit!("   ucc help <command>        Get help on a command").to_vec());
    let order: Vec<Cell<usize>> = (0..items.len()).map(Cell::new).collect();
    let text = |slot: isize| items[order[slot as usize].get()].as_ptr();
    let _ = quicksort::<Infallible>(items.len() as isize, |_| Ok(()),
        |a, b| Ok(stricmp(text(a), text(b))), |a, b| order[a as usize].swap(&order[b as usize]));
    for slot in &order { host.warn_log(&items[slot.get()])?; }
    Ok(())
}

fn run_command<H: UccHost>(host: &H, list: *const TArrayRaw, token: &[u32], flags: u32, help: bool) -> Result<(), H::Error> {
    let mut class = host.load_commandlet_class(token, flags)?;
    let suffixed = unsafe { fmt(&[A::P(token.as_ptr()), A::L("Commandlet")]) };
    if class.is_null() { class = host.load_commandlet_class(&suffixed, flags)?; }
    if class.is_null() {
        let mut selected = None;
        let mut index = 0;
        while index < unsafe { registry_len(list) } {
            let name = unsafe { registry_name(list, index) };
            let short = unsafe { short_name(name.as_ptr()) };
            if stricmp(token.as_ptr(), short) == 0 || stricmp(suffixed.as_ptr(), short) == 0 {
                selected = Some(name);
                break;
            }
            index += 1;
        }
        if let Some(name) = selected {
            class = host.load_commandlet_class(&name, flags)?;
        }
    }
    if class.is_null() {
        banner(host)?;
        host.warn_logf(&unsafe { fmt(&[A::L("Commandlet "), A::P(token.as_ptr()), A::L(" not found")]) })?;
        return Ok(());
    }
    let default = host.default_object(class)?.cast::<UCommandlet>();
    if help { show_help(host, default) } else { execute(host, class, default) }
}

fn show_help<H: UccHost>(host: &H, default: *const UCommandlet) -> Result<(), H::Error> {
    unsafe {
        let usage = ptr::addr_of!((*default).help_usage);
        if !fstring_is_empty(usage) {
            host.warn_logf(lit!("Usage:"))?;
            host.warn_logf(&fmt(&[A::L("   ucc "), A::P(fstring_text(usage))]))?;
        }
        let parms = ptr::addr_of!((*default).help_parm).cast::<TArrayRaw>();
        let descs = ptr::addr_of!((*default).help_desc).cast::<TArrayRaw>();
        if !fstring_is_empty(parms) {
            host.warn_logf(lit!(""))?;
            host.warn_logf(lit!("Parameters:"))?;
            for index in 0..(*default).help_parm.len() {
                let parm = parms.add(index);
                if fstring_is_empty(parm) { break; }
                let padded = right_pad(parm, PARAMETER_PAD);
                host.warn_logf(&fmt(&[A::L("   "), A::P(padded.as_ptr()), A::L(" "), A::P(fstring_text(descs.add(index))) ]))?;
            }
        }
        let link = ptr::addr_of!((*default).help_web_link);
        if !fstring_is_empty(link) {
            host.warn_logf(lit!(""))?;
            host.warn_logf(lit!("For more info, see"))?;
            host.warn_logf(&fmt(&[A::L("   "), A::P(fstring_text(link))]))?;
        }
    }
    Ok(())
}
fn execute<H: UccHost>(host: &H, class: *mut c_void, default: *const UCommandlet) -> Result<(), H::Error> {
    unsafe {
        let log_to_stdout = (*default).log_to_stdout();
        if log_to_stdout { host.set_log_to_stdout(true)?; }
        if (*default).show_banner() { banner(host)?; }
        host.with_class_full_name(class, |name| host.debug_log(&fmt(&[A::L("Executing "), A::P(name)])))?;
        host.set_commandlet_mode((*default).is_client(), (*default).is_server(), (*default).is_editor(), (*default).lazy_load())?;
        let commandlet = host.construct_commandlet(class)?;
        host.init_execution(commandlet)?;
        host.parse_parms(commandlet, host.command_line()?)?;
        let _main_result = host.main(commandlet, host.command_line()?)?;
        if (*commandlet.cast::<UCommandlet>()).show_error_count() {
            host.gwarn_logf(&fmt(&[A::L("Success - 0 error(s), "), A::I(host.warning_count()), A::L(" warnings")]))?;
        }
        if log_to_stdout { host.set_log_to_stdout(false)?; }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
