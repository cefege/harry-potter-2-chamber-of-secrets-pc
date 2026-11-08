//! `Localize` (`UnMisc.cpp`): the lookup behind every `Localize*` overload.
//!
//! The result is the next `appStaticString1024` slot. Before startup, or
//! without a config cache, it is a copy of `Key`. Otherwise it is
//! `Section`/`Key` read from `<Package>.<LangExt>`, retried once as
//! `<Package>.int`. Unless `Optional`, a miss logs under `NAME_Localization`
//! and yields `<?LangExt?Package.Section.Key?>`. The engine state it reads
//! (`GIsStarted`, `GConfig`, the active language, `debugf`) stays behind
//! native leaves.

use std::ffi::c_void;
use std::ptr::null;

use crate::ansi::{self, STATIC_STRING_LEN, Tchar};
use crate::printf_format::{append_text, put};

/// The `Count` that `appSprintf` (`UnAnsi.cpp`) passes to
/// `appVswprintfCompat`.
const SPRINTF_COUNT: usize = 1024;

static INT: [Tchar; 4] = ansi::wide(b"int\0");
static NULL_TEXT: [Tchar; 7] = ansi::wide(b"(null)\0");
static DOT: [Tchar; 2] = ansi::wide(b".\0");
static QUESTION: [Tchar; 2] = ansi::wide(b"?\0");
static MISSING_OPEN: [Tchar; 3] = ansi::wide(b"<?\0");
static MISSING_CLOSE: [Tchar; 3] = ansi::wide(b"?>\0");

/// Native leaves, filled in `Core/Src/UnMisc.cpp`. A nonzero status means the
/// leaf captured a C++ exception.
#[repr(C)]
pub struct LocalizeServices {
    /// `GIsStarted && GConfig`.
    pub config_ready: unsafe extern "C" fn() -> i32,
    /// Writes `UObject::GetLanguage()` to `*out`.
    pub language: unsafe extern "C" fn(out: *mut *const c_void) -> i32,
    /// `GConfig->GetString(Section, Key, Value, Size, Filename)`, its `UBOOL`
    /// written to `*found`.
    pub get_string: unsafe extern "C" fn(
        section: *const c_void,
        key: *const c_void,
        value: *mut c_void,
        size: i32,
        filename: *const c_void,
        found: *mut i32,
    ) -> i32,
    /// `debugf(NAME_Localization, TEXT("No localization: %s.%s.%s (%s)"),
    /// Package, Section, Key, LangExt)`.
    pub log_missing: unsafe extern "C" fn(
        package: *const c_void,
        section: *const c_void,
        key: *const c_void,
        lang_ext: *const c_void,
    ) -> i32,
}

/// `appSprintf(dest, ...)` for a format of `%s` directives and literals,
/// given as consecutive texts. A NULL text prints `(null)`. On overflow `dest`
/// becomes empty, as `appVswprintfCompat` leaves it.
///
/// # Safety
/// `dest` is writable for [`SPRINTF_COUNT`] elements; each non-NULL text is
/// NUL-terminated.
unsafe fn sprintf_texts(dest: *mut Tchar, texts: &[*const Tchar]) {
    let mut out = 0;
    for &text in texts {
        let text = if text.is_null() {
            NULL_TEXT.as_ptr()
        } else {
            text
        };
        if !unsafe { append_text(dest, SPRINTF_COUNT, &mut out, text) } {
            unsafe { dest.write(0) };
            return;
        }
    }
    unsafe { dest.add(out).write(0) };
}

/// Copies `key` into a ring slot, truncated to the slot.
///
/// # Safety
/// `dest` is a ring slot; `key` is NUL-terminated.
unsafe fn copy_key(dest: *mut Tchar, key: *const Tchar) {
    let mut out = 0;
    let mut index = 0;
    loop {
        let ch = unsafe { key.add(index).read() };
        if ch == 0 || !unsafe { put(dest, STATIC_STRING_LEN, &mut out, ch) } {
            break;
        }
        index += 1;
    }
    unsafe { dest.add(out).write(0) };
}

/// `Localize(Section, Key, Package, LangExt, Optional)`; writes the ring slot
/// to `*out` before any leaf runs. Returns the first nonzero leaf status, with
/// the slot holding whatever that leaf left.
///
/// Before startup, a `Key` longer than the slot is truncated where the
/// original `appStrcpy` overran it. The filename uses `appSprintf`'s own
/// bound: the original wrote it into a 256-element local under that bound.
///
/// # Safety
/// Engine-thread serialized (see [`ansi::static_string_1024`]). `key` is
/// NUL-terminated. Once the config cache is ready, `section` and `key` are
/// what `GetString` accepts; `package` and `lang_ext` are NULL or
/// NUL-terminated, and the active language is NUL-terminated.
pub unsafe fn localize(
    services: &LocalizeServices,
    section: *const Tchar,
    key: *const Tchar,
    package: *const Tchar,
    lang_ext: *const Tchar,
    optional: bool,
    out: &mut *const Tchar,
) -> i32 {
    let result = unsafe { ansi::static_string_1024() };
    *out = result;
    if unsafe { (services.config_ready)() } == 0 {
        unsafe { copy_key(result, key) };
        return 0;
    }
    let mut lang = lang_ext;
    if lang.is_null() {
        let mut active = null();
        let status = unsafe { (services.language)(&mut active) };
        if status != 0 {
            return status;
        }
        lang = active.cast();
    }
    let mut filename = [0; SPRINTF_COUNT];
    loop {
        unsafe { sprintf_texts(filename.as_mut_ptr(), &[package, DOT.as_ptr(), lang]) };
        let mut found = 0;
        let status = unsafe {
            (services.get_string)(
                section.cast(),
                key.cast(),
                result.cast(),
                STATIC_STRING_LEN as i32,
                filename.as_ptr().cast(),
                &mut found,
            )
        };
        if status != 0 || found != 0 {
            return status;
        }
        if unsafe { ansi::stricmp(lang, INT.as_ptr()) } != 0 {
            lang = INT.as_ptr();
            continue;
        }
        if optional {
            return 0;
        }
        let status = unsafe {
            (services.log_missing)(package.cast(), section.cast(), key.cast(), lang.cast())
        };
        if status != 0 {
            return status;
        }
        unsafe {
            sprintf_texts(
                result,
                &[
                    MISSING_OPEN.as_ptr(),
                    lang,
                    QUESTION.as_ptr(),
                    package,
                    DOT.as_ptr(),
                    section,
                    DOT.as_ptr(),
                    key,
                    MISSING_CLOSE.as_ptr(),
                ],
            )
        };
        return 0;
    }
}

#[cfg(test)]
mod tests {
    use super::{LocalizeServices, SPRINTF_COUNT, localize};
    use crate::ansi::{STATIC_STRING_LEN, Tchar};
    use std::cell::RefCell;
    use std::ffi::c_void;
    use std::ptr::null;
    use std::sync::Mutex;

    /// The result ring is process-global; these tests take turns on it.
    static RING: Mutex<()> = Mutex::new(());

    fn wide(text: &str) -> Vec<Tchar> {
        text.chars().map(|c| c as Tchar).chain([0]).collect()
    }

    unsafe fn read(text: *const c_void) -> String {
        let text = text.cast::<Tchar>();
        let mut result = String::new();
        let mut index = 0;
        loop {
            let ch = unsafe { text.add(index).read() };
            if ch == 0 {
                return result;
            }
            result.push(char::from_u32(ch as u32).unwrap());
            index += 1;
        }
    }

    #[derive(Default)]
    struct Mock {
        ready: bool,
        language: Vec<Tchar>,
        language_status: i32,
        /// Filename a lookup succeeds for, and the value it writes.
        hit: Option<(String, String)>,
        get_status: i32,
        log_status: i32,
        lookups: Vec<(String, String, String, i32)>,
        logs: Vec<[String; 4]>,
    }

    thread_local! {
        static MOCK: RefCell<Mock> = RefCell::new(Mock::default());
    }

    unsafe extern "C" fn ready() -> i32 {
        MOCK.with_borrow(|m| m.ready as i32)
    }

    unsafe extern "C" fn language(out: *mut *const c_void) -> i32 {
        MOCK.with_borrow(|m| {
            unsafe { out.write(m.language.as_ptr().cast()) };
            m.language_status
        })
    }

    unsafe extern "C" fn get_string(
        section: *const c_void,
        key: *const c_void,
        value: *mut c_void,
        size: i32,
        filename: *const c_void,
        found: *mut i32,
    ) -> i32 {
        MOCK.with_borrow_mut(|m| {
            let filename = unsafe { read(filename) };
            m.lookups.push((
                unsafe { read(section) },
                unsafe { read(key) },
                filename.clone(),
                size,
            ));
            let value = value.cast::<Tchar>();
            unsafe { value.write(0) };
            if m.get_status != 0 {
                return m.get_status;
            }
            if let Some((hit, text)) = &m.hit
                && *hit == filename
            {
                for (index, &ch) in wide(text).iter().enumerate() {
                    unsafe { value.add(index).write(ch) };
                }
                unsafe { found.write(1) };
            } else {
                unsafe { found.write(0) };
            }
            0
        })
    }

    unsafe extern "C" fn log_missing(
        package: *const c_void,
        section: *const c_void,
        key: *const c_void,
        lang_ext: *const c_void,
    ) -> i32 {
        MOCK.with_borrow_mut(|m| {
            m.logs
                .push(unsafe { [read(package), read(section), read(key), read(lang_ext)] });
            m.log_status
        })
    }

    const SERVICES: LocalizeServices = LocalizeServices {
        config_ready: ready,
        language,
        get_string,
        log_missing,
    };

    fn run(mock: Mock, lang_ext: Option<&str>, optional: bool) -> (i32, String, Mock) {
        run_in("Core", mock, lang_ext, optional)
    }

    fn run_in(
        package: &str,
        mock: Mock,
        lang_ext: Option<&str>,
        optional: bool,
    ) -> (i32, String, Mock) {
        let _ring = RING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        MOCK.set(mock);
        let (section, key, package) = (wide("Errors"), wide("Missing"), wide(package));
        let lang_ext = lang_ext.map(wide);
        let lang_ptr = lang_ext.as_ref().map_or(null(), |l| l.as_ptr());
        let mut out = null();
        let status = unsafe {
            localize(
                &SERVICES,
                section.as_ptr(),
                key.as_ptr(),
                package.as_ptr(),
                lang_ptr,
                optional,
                &mut out,
            )
        };
        let text = unsafe { read(out.cast()) };
        (status, text, MOCK.take())
    }

    fn ready_mock(language: &str) -> Mock {
        Mock {
            ready: true,
            language: wide(language),
            ..Mock::default()
        }
    }

    #[test]
    fn before_startup_returns_key_without_leaves() {
        let (status, text, mock) = run(
            Mock {
                language_status: 9,
                ..Mock::default()
            },
            None,
            false,
        );
        assert_eq!((status, text.as_str()), (0, "Missing"));
        assert!(mock.lookups.is_empty() && mock.logs.is_empty());
    }

    #[test]
    fn active_language_hit_reads_one_file() {
        let mock = Mock {
            hit: Some(("Core.det".into(), "Fehlt".into())),
            ..ready_mock("det")
        };
        let (status, text, mock) = run(mock, None, false);
        assert_eq!((status, text.as_str()), (0, "Fehlt"));
        let size = STATIC_STRING_LEN as i32;
        assert_eq!(
            mock.lookups,
            [("Errors".into(), "Missing".into(), "Core.det".into(), size)]
        );
    }

    #[test]
    fn miss_retries_int_then_marks_and_logs() {
        let (status, text, mock) = run(ready_mock("det"), None, false);
        assert_eq!((status, text.as_str()), (0, "<?int?Core.Errors.Missing?>"));
        let files: Vec<_> = mock.lookups.iter().map(|l| l.2.as_str()).collect();
        assert_eq!(files, ["Core.det", "Core.int"]);
        assert_eq!(
            mock.logs,
            [["Core", "Errors", "Missing", "int"].map(String::from)]
        );
    }

    #[test]
    fn int_is_matched_case_insensitively_and_not_retried() {
        let (_, text, mock) = run(ready_mock("det"), Some("INT"), false);
        assert_eq!(text, "<?INT?Core.Errors.Missing?>");
        assert_eq!(mock.lookups.len(), 1);
    }

    #[test]
    fn optional_miss_keeps_lookup_output_and_skips_log() {
        let (status, text, mock) = run(ready_mock("det"), None, true);
        assert_eq!((status, text.as_str()), (0, ""));
        assert!(mock.logs.is_empty());
    }

    #[test]
    fn leaf_status_stops_lookup() {
        let mock = Mock {
            language_status: 7,
            ..ready_mock("det")
        };
        assert_eq!(run(mock, None, false).0, 7);
        let (status, _, mock) = run(
            Mock {
                get_status: 5,
                ..ready_mock("det")
            },
            None,
            false,
        );
        assert_eq!((status, mock.lookups.len()), (5, 1));
        let (status, text, _) = run(
            Mock {
                log_status: 3,
                ..ready_mock("det")
            },
            None,
            false,
        );
        assert_eq!((status, text.as_str()), (3, ""));
    }

    #[test]
    fn marker_fills_to_appsprintf_bound_then_empties() {
        // The marker around `Package`.
        let marker_extra = "<?int?".len() + ".Errors.Missing?>".len();
        let fits = "p".repeat(SPRINTF_COUNT - 1 - marker_extra);
        let (_, text, _) = run_in(&fits, ready_mock("det"), None, false);
        assert_eq!(text, format!("<?int?{fits}.Errors.Missing?>"));
        assert_eq!(text.len(), SPRINTF_COUNT - 1);

        let over = format!("{fits}p");
        let (status, text, mock) = run_in(&over, ready_mock("det"), None, false);
        assert_eq!((status, text.as_str()), (0, ""));
        let files: Vec<_> = mock.lookups.iter().map(|l| l.2.clone()).collect();
        assert_eq!(files, [format!("{over}.det"), format!("{over}.int")]);
    }
}
impl crate::core_access::CoreAccess<'_> {
    /// Returns a localized value, falling back to the international package file.
    pub fn localize_text(
        self,
        section: &[u32],
        key: &[u32],
        package: Option<&[u32]>,
        lang_ext: Option<&[u32]>,
        optional: bool,
    ) -> crate::core_access::RuntimeResult<Vec<u32>> {
        if self.core_package().is_null() {
            return Ok(key.iter().copied().take_while(|&unit| unit != 0).take(1023).collect());
        }
        let package = package.unwrap_or(&[40, 110, 117, 108, 108, 41]);
        let config = self.config_storage();
        let active_language = unsafe { (&*config).language_text() };
        let requested_language = lang_ext
            .map(|text| text.iter().copied().take_while(|&unit| unit != 0).collect())
            .unwrap_or(active_language);
        let mut selected_language = requested_language.clone();
        loop {
            let filename = path_text(&unsafe { &*config }
                .localization_filename_for(package, Some(&selected_language))?);
            if let Some(value) = unsafe { &mut *config }.get(Some(&filename), section, key)? {
                return Ok(value.into_iter().take(1023).collect());
            }
            if !ascii_equal(&selected_language, &[105, 110, 116]) {
                selected_language = vec![105, 110, 116];
                continue;
            }
            break;
        }
        if optional { return Ok(Vec::new()); }
        let mut message = Vec::new();
        message.extend("No localization: ".encode_utf16().map(u32::from));
        append(&mut message, package);
        message.push(46);
        append(&mut message, section);
        message.push(46);
        append(&mut message, key);
        message.extend(" (".encode_utf16().map(u32::from));
        append(&mut message, &selected_language);
        message.push(41);
        let category = self.intern_str("Localization", crate::name_owner::FindName::Add)?;
        self.emit_log(category, &message)?;
        let mut missing = vec![60, 63];
        append(&mut missing, &selected_language);
        missing.push(63);
        append(&mut missing, package);
        missing.push(46);
        append(&mut missing, section);
        missing.push(46);
        append(&mut missing, key);
        if missing.len() > 1023 {
            return Ok(Vec::new());
        }
        Ok(missing)
    }
    /// Returns the configured Core language as an owned UTF-32 snapshot.
    pub fn language_text(self) -> crate::core_access::RuntimeResult<Vec<u32>> {
        Ok(unsafe { &*self.config_storage() }.language_text())
    }

    /// Changes language and reloads localized values on the live object graph.
    pub fn set_language(self, language: &[u32]) -> crate::core_access::RuntimeResult<()> {
        let config = self.config_storage();
        let previous = unsafe { (&*config).language_text() };
        unsafe { &mut *config }.set_language(language)?;
        let current = unsafe { (&*config).language_text() };
        if ascii_equal(&previous, &current) {
            return Ok(());
        }
        let mut index = -1;
        loop {
            // SAFETY: the table publishes each cursor step before any reentrant load.
            unsafe {
                crate::object_storage::next_index(
                    self.storage(),
                    &mut index,
                    std::ptr::null_mut(),
                    any_object,
                )
            }
            .map_err(|status| crate::core_access::RuntimeError::new("Object iteration failed")
                .with_context(&format!("status {status}")))?;
            if index >= self.object_count() { break; }
            let object = self.object_at(index);
            if !object.is_null() { unsafe { self.load_localized(object)?; } }
        }
        self.flush_localized()
    }
}

fn ascii_equal(a: &[u32], b: &[u32]) -> bool {
    a.iter().copied().take_while(|&unit| unit != 0)
        .map(lower_ascii_unit)
        .eq(b.iter().copied().take_while(|&unit| unit != 0).map(lower_ascii_unit))
}

fn lower_ascii_unit(unit: u32) -> u32 {
    if (65..=90).contains(&unit) { unit + 32 } else { unit }
}
fn path_text(path: &std::path::Path) -> Vec<u32> {
    path.to_string_lossy().encode_utf16().map(u32::from).collect()
}
fn append(output: &mut Vec<u32>, input: &[u32]) {
    output.extend(input.iter().copied().take_while(|&unit| unit != 0));
}
unsafe extern "C" fn any_object(_: *mut std::ffi::c_void, _: *mut std::ffi::c_void, found: *mut i32) -> i32 {
    unsafe { found.write(1); }
    0
}
