use super::*;
use std::ffi::{CStr, CString};

const LOAD_QUIET: u32 = 0x2002;
const LOAD_DISALLOW_FILES: u32 = 0x0040;
const LOAD_NO_FAIL: u32 = 0x0001;

fn prepare(args: &[&[u8]]) -> Result<(Vec<u32>, Vec<u32>), i32> {
    let strings: Vec<CString> = args
        .iter()
        .map(|arg| CString::new(*arg).expect("test argument has no interior NUL"))
        .collect();
    let pointers: Vec<*const c_char> = strings.iter().map(|arg| arg.as_ptr()).collect();
    unsafe {
        prepare_arguments(pointers.len() as i32, pointers.as_ptr(), &|argument| {
            let bytes = unsafe { std::ffi::CStr::from_ptr(argument) }.to_bytes();
            bytes.starts_with(b"-datadir=")
        })
    }
}

#[test]
fn filters_data_directory_arguments_without_changing_parameter_order() {
    let (command, parameters) = prepare(&[
        b"ucc",
        b"-datadir=/first",
        b"HelloWorld",
        b"-datadir=/second",
        b"intparm=73",
        b"strparm=two words",
    ])
    .expect("valid input");
    assert_eq!(command, "HelloWorld\0".encode_utf16().map(u32::from).collect::<Vec<_>>());
    assert_eq!(parameters, "intparm=73 strparm=two words\0".encode_utf16().map(u32::from).collect::<Vec<_>>());
}

#[test]
fn accepts_exact_maximum_terminated_command_line_and_rejects_one_unit_more() {
    let max = vec![b'x'; TEXT_CAPACITY - 1];
    let c_string = CString::new(max.clone()).expect("bytes have no NUL");
    let program = CString::new("ucc").expect("literal has no NUL");
    let command = CString::new("test").expect("literal has no NUL");
    let argv = [program.as_ptr(), command.as_ptr(), c_string.as_ptr()];
    let accepted = unsafe {
        prepare_arguments(argv.len() as i32, argv.as_ptr(), &|_| false)
    };
    assert!(accepted.is_ok(), "1023 UTF-32 units plus the terminator fit");

    let too_long = vec![b'x'; TEXT_CAPACITY];
    let c_string = CString::new(too_long).expect("bytes have no NUL");
    let argv = [program.as_ptr(), command.as_ptr(), c_string.as_ptr()];
    let rejected = unsafe { prepare_arguments(argv.len() as i32, argv.as_ptr(), &|_| false) };
    assert_eq!(rejected, Err(1));
}

#[test]
fn rejects_invalid_utf8_command_and_parameters_before_startup() {
    let invalid = CString::new([0xff]).expect("byte has no NUL");
    let program = CString::new("ucc").expect("literal has no NUL");
    let argv = [program.as_ptr(), invalid.as_ptr()];
    assert_eq!(unsafe { prepare_arguments(argv.len() as i32, argv.as_ptr(), &|_| false) }, Err(1));

    let command = CString::new("HelloWorld").expect("literal has no NUL");
    let argv = [program.as_ptr(), command.as_ptr(), invalid.as_ptr()];
    assert_eq!(unsafe { prepare_arguments(argv.len() as i32, argv.as_ptr(), &|_| false) }, Err(1));
}

#[test]
fn measures_multibyte_arguments_in_utf32_units() {
    let accepted_bytes = "é".repeat(TEXT_CAPACITY - 1).into_bytes();
    let accepted = prepare(&[b"ucc", b"test", &accepted_bytes]).expect("1023 decoded units fit");
    assert_eq!(accepted.1.len(), TEXT_CAPACITY);
    assert!(accepted.1[..TEXT_CAPACITY - 1].iter().all(|&unit| unit == 'é' as u32));

    let rejected_bytes = "é".repeat(TEXT_CAPACITY).into_bytes();
    assert_eq!(prepare(&[b"ucc", b"test", &rejected_bytes]), Err(1));
}

struct PolicyHost {
    class_name: String,
    default: Box<UCommandlet>,
    instance: Box<UCommandlet>,
    app_line: Vec<u32>,
    loads: std::cell::RefCell<Vec<(String, u32)>>,
    mode: std::cell::Cell<(bool, bool, bool, bool)>,
    editor: std::cell::Cell<Option<bool>>,
    log_routing: std::cell::RefCell<Vec<bool>>,
    executed: std::cell::Cell<bool>,
    exited: std::cell::Cell<bool>,
}

impl PolicyHost {
    fn new(class_name: &str, app_line: &str) -> Self {
        // SAFETY: UCommandlet is an ABI record whose zeroed FString/TArray fields are empty.
        let mut default = Box::new(unsafe { std::mem::zeroed::<UCommandlet>() });
        // SAFETY: The boxed record remains at a stable address for this test host's lifetime.
        let instance = Box::new(unsafe { std::mem::zeroed::<UCommandlet>() });
        default.set_is_server(true);
        default.set_lazy_load(true);
        default.set_log_to_stdout(true);
        Self {
            class_name: class_name.to_owned(),
            default,
            instance,
            app_line: app_line.encode_utf16().map(u32::from).chain([0]).collect(),
            loads: std::cell::RefCell::new(Vec::new()),
            mode: std::cell::Cell::new((true, true, true, false)),
            editor: std::cell::Cell::new(None),
            log_routing: std::cell::RefCell::new(Vec::new()),
            executed: std::cell::Cell::new(false),
            exited: std::cell::Cell::new(false),
        }
    }

    fn class(&self) -> *mut c_void {
        (&*self.default as *const UCommandlet).cast_mut().cast()
    }
}

impl UccHost for PolicyHost {
    type Error = std::convert::Infallible;

    fn app_init(&self, _command_line: &[u32]) -> Result<(), Self::Error> { Ok(()) }
    fn set_language(&self, _language: &[u32]) -> Result<(), Self::Error> { Ok(()) }
    fn with_registry<T>(
        &self,
        body: impl FnOnce(*const TArrayRaw) -> Result<T, Self::Error>,
    ) -> Result<T, Self::Error> {
        body(std::ptr::null())
    }
    fn check_failed(&self, _expr: &CStr, _line: i32) -> Result<(), Self::Error> { Ok(()) }
    fn commandlet_class(&self) -> Result<*mut c_void, Self::Error> { Ok(self.class()) }
    fn load_commandlet_class(&self, name: &[u32], flags: u32) -> Result<*mut c_void, Self::Error> {
        let name: String = name.iter().take_while(|&&unit| unit != 0)
            .filter_map(|&unit| char::from_u32(unit)).collect();
        self.loads.borrow_mut().push((name.clone(), flags));
        Ok(if name.eq_ignore_ascii_case(&self.class_name) || name == "Core.Commandlet" {
            self.class()
        } else {
            std::ptr::null_mut()
        })
    }
    fn command_line(&self) -> Result<*const u32, Self::Error> { Ok(self.app_line.as_ptr()) }
    fn default_object(&self, _class: *mut c_void) -> Result<*mut c_void, Self::Error> {
        Ok(self.class())
    }
    fn with_class_full_name<T>(
        &self,
        _class: *mut c_void,
        body: impl FnOnce(*const u32) -> Result<T, Self::Error>,
    ) -> Result<T, Self::Error> {
        let name: Vec<u32> = "Core.TestCommandlet\0"
            .encode_utf16()
            .map(u32::from)
            .collect();
        body(name.as_ptr())
    }
    fn construct_commandlet(&self, _class: *mut c_void) -> Result<*mut c_void, Self::Error> {
        Ok((&*self.instance as *const UCommandlet).cast_mut().cast())
    }
    fn init_execution(&self, _commandlet: *mut c_void) -> Result<(), Self::Error> { Ok(()) }
    fn parse_parms(&self, _commandlet: *mut c_void, _parms: *const u32) -> Result<(), Self::Error> {
        Ok(())
    }
    fn main(&self, _commandlet: *mut c_void, _parms: *const u32) -> Result<i32, Self::Error> {
        self.executed.set(true);
        Ok(29)
    }
    fn warn_log(&self, _text: &[u32]) -> Result<(), Self::Error> { Ok(()) }
    fn warn_logf(&self, _text: &[u32]) -> Result<(), Self::Error> { Ok(()) }
    fn gwarn_logf(&self, _text: &[u32]) -> Result<(), Self::Error> { Ok(()) }
    fn debug_log(&self, _text: &[u32]) -> Result<(), Self::Error> { Ok(()) }
    fn pre_exit(&self) -> Result<(), Self::Error> {
        self.exited.set(true);
        Ok(())
    }
    fn set_startup_mode(&self) -> Result<(), Self::Error> {
        self.mode.set((true, true, true, false));
        Ok(())
    }
    fn set_editor(&self, editor: bool) -> Result<(), Self::Error> {
        self.editor.set(Some(editor));
        Ok(())
    }
    fn set_commandlet_mode(
        &self, client: bool, server: bool, editor: bool, lazy: bool,
    ) -> Result<(), Self::Error> {
        self.mode.set((client, server, editor, lazy));
        Ok(())
    }
    fn set_log_to_stdout(&self, enabled: bool) -> Result<(), Self::Error> {
        self.log_routing.borrow_mut().push(enabled);
        Ok(())
    }
    fn warning_count(&self) -> i32 { 0 }
    fn load_quiet(&self) -> u32 { LOAD_QUIET }
    fn load_disallow_files(&self) -> u32 { LOAD_DISALLOW_FILES }
    fn load_no_fail(&self) -> u32 { LOAD_NO_FAIL }
}

fn run_policy(host: &PolicyHost, arguments: &[&str]) -> Option<i32> {
    let strings: Vec<CString> = std::iter::once("ucc")
        .chain(arguments.iter().copied())
        .map(|argument| CString::new(argument).expect("test argument has no NUL"))
        .collect();
    let pointers: Vec<*const c_char> = strings.iter().map(|argument| argument.as_ptr()).collect();
    // SAFETY: The CStrings and pointer array stay live for this synchronous session.
    unsafe { run_session(host, pointers.len() as i32, pointers.as_ptr(), |_| false) }
        .expect("test host cannot fail")
}

#[test]
fn make_load_flags_and_commandlet_settings_drive_execution() {
    let host = PolicyHost::new("MakeCommandlet", "");
    assert_eq!(run_policy(&host, &["make"]), None);
    assert_eq!(
        host.loads.borrow().as_slice(),
        [("make".to_owned(), LOAD_QUIET | LOAD_DISALLOW_FILES),
            ("makeCommandlet".to_owned(), LOAD_QUIET | LOAD_DISALLOW_FILES)]
    );
    assert_eq!(host.mode.get(), (false, true, false, true));
    assert_eq!(&*host.log_routing.borrow(), &[true, false]);
    assert!(host.executed.get());
    assert!(host.exited.get());
}

#[test]
fn help_keyword_precedes_command_selection_and_uses_app_command_line() {
    let host = PolicyHost::new("TestCommandlet", "test");
    assert_eq!(run_policy(&host, &["HELP"]), None);
    assert_eq!(host.loads.borrow()[0], ("Core.Commandlet".to_owned(), LOAD_NO_FAIL));
    assert_eq!(host.loads.borrow()[1], ("test".to_owned(), LOAD_QUIET));
    assert_eq!(host.loads.borrow()[2], ("testCommandlet".to_owned(), LOAD_QUIET));
    assert_eq!(host.editor.get(), Some(false));
    assert!(!host.executed.get());
    assert!(host.exited.get());
}
