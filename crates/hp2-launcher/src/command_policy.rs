//! Argument forwarding and SDL status selection. Native services retain live
//! argv, stack TCHAR buffers and FString allocation/lifetimes. No owner borrow,
//! argument snapshot, allocation or lock is held across a callback.
use std::ffi::c_void;

/// One live native query, conversion, append or diagnostic.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2LauncherCommandLeaf {
    IsDataDirectory,
    ConvertArgument,
    ReportInvalid,
    CommandLineLength,
    AppendSeparator,
    AppendArgument,
    ConvertStatus,
}

/// Each callback contains native exceptions/signals and returns their first
/// failure as nonzero status. The native caller rethrows only after Rust returns.
/// Reentry is allowed; each call has independent buffers and exception state.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2LauncherCommandServices {
    pub context: *mut c_void,
    pub invoke:
        Option<unsafe extern "C" fn(*mut c_void, Hp2LauncherCommandLeaf, i32, *mut i32) -> i32>,
}

unsafe fn invoke(
    services: &Hp2LauncherCommandServices,
    leaf: Hp2LauncherCommandLeaf,
    index: i32,
) -> Result<i32, i32> {
    let callback = services.invoke.ok_or(1)?;
    let mut value = 0;
    let status = unsafe { callback(services.context, leaf, index, &mut value) };
    if status == 0 { Ok(value) } else { Err(status) }
}

/// Forward original argv[1..argc], rereading each argument at its original
/// native use site. Accepted empty arguments still append a separator whenever
/// the accumulated string is nonempty; leading empties do not create one.
/// # Safety
/// Services and their native context/buffers satisfy the checked callback and
/// reentry contract through return. Each leaf performs only its named action.
pub unsafe fn forward(services: &Hp2LauncherCommandServices, argc: i32) -> Result<(), i32> {
    use Hp2LauncherCommandLeaf::*;
    for index in 1..argc {
        if unsafe { invoke(services, IsDataDirectory, index)? } != 0 {
            continue;
        }
        if unsafe { invoke(services, ConvertArgument, index)? } == 0 {
            unsafe { invoke(services, ReportInvalid, index)? };
            continue;
        }
        if unsafe { invoke(services, CommandLineLength, index)? } != 0 {
            unsafe { invoke(services, AppendSeparator, index)? };
        }
        unsafe { invoke(services, AppendArgument, index)? };
    }
    Ok(())
}

/// Select the native converted buffer only for nonnull, convertible raw status.
/// The native caller constructs its one FString after this function returns.
/// # Safety
/// Services satisfy the checked callback contract. ConvertStatus uses the live
/// raw pointer and original 512-TCHAR stack buffer; null must not be converted.
pub unsafe fn status_buffer(
    services: &Hp2LauncherCommandServices,
    has_raw: bool,
) -> Result<bool, i32> {
    Ok(has_raw && unsafe { invoke(services, Hp2LauncherCommandLeaf::ConvertStatus, 0)? } != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    fn leaf_name(leaf: Hp2LauncherCommandLeaf) -> &'static str {
        use Hp2LauncherCommandLeaf::*;
        match leaf {
            IsDataDirectory => "IsDataDirectory",
            ConvertArgument => "ConvertArgument",
            ReportInvalid => "ReportInvalid",
            CommandLineLength => "CommandLineLength",
            AppendSeparator => "AppendSeparator",
            AppendArgument => "AppendArgument",
            ConvertStatus => "ConvertStatus",
        }
    }

    thread_local! {
        static CALLS: RefCell<Vec<(&'static str, i32)>> = const { RefCell::new(Vec::new()) };
        static SCRIPT: RefCell<HashMap<&'static str, i32>> = RefCell::new(HashMap::new());
        /// Per-`(leaf, index)` overrides, consulted before the leaf-only
        /// `SCRIPT` default -- lets a single `forward()` call exercise a
        /// mixed argv where different argument indices take different
        /// branches (skip / fail / grow the command line).
        static SCRIPT_AT: RefCell<HashMap<(&'static str, i32), i32>> = RefCell::new(HashMap::new());
        static FAIL: RefCell<Option<&'static str>> = const { RefCell::new(None) };
    }

    unsafe extern "C" fn fake_call(
        _ctx: *mut c_void,
        leaf: Hp2LauncherCommandLeaf,
        index: i32,
        out: *mut i32,
    ) -> i32 {
        let name = leaf_name(leaf);
        CALLS.with(|c| c.borrow_mut().push((name, index)));
        if FAIL.with(|f| *f.borrow() == Some(name)) {
            return 7;
        }
        let value = SCRIPT_AT
            .with(|s| s.borrow().get(&(name, index)).copied())
            .unwrap_or_else(|| SCRIPT.with(|s| s.borrow().get(name).copied().unwrap_or(0)));
        unsafe { *out = value };
        0
    }

    fn reset() {
        CALLS.with(|c| c.borrow_mut().clear());
        SCRIPT.with(|s| s.borrow_mut().clear());
        SCRIPT_AT.with(|s| s.borrow_mut().clear());
        FAIL.with(|f| *f.borrow_mut() = None);
    }

    fn script(entries: &[(&'static str, i32)]) {
        SCRIPT.with(|s| {
            let mut s = s.borrow_mut();
            for (k, v) in entries {
                s.insert(*k, *v);
            }
        });
    }

    fn script_at(entries: &[(&'static str, i32, i32)]) {
        SCRIPT_AT.with(|s| {
            let mut s = s.borrow_mut();
            for (k, index, v) in entries {
                s.insert((*k, *index), *v);
            }
        });
    }

    fn fail(leaf: &'static str) {
        FAIL.with(|f| *f.borrow_mut() = Some(leaf));
    }

    fn services() -> Hp2LauncherCommandServices {
        Hp2LauncherCommandServices {
            context: std::ptr::null_mut(),
            invoke: Some(fake_call),
        }
    }

    fn calls() -> Vec<(&'static str, i32)> {
        CALLS.with(|c| c.borrow().clone())
    }

    #[test]
    fn forward_skips_data_directory_arguments_entirely() {
        reset();
        script(&[("IsDataDirectory", 1)]);
        let result = unsafe { forward(&services(), 2) };
        assert_eq!(result, Ok(()));
        assert_eq!(calls(), vec![("IsDataDirectory", 1)]);
    }

    #[test]
    fn forward_reports_invalid_and_continues_when_conversion_fails() {
        reset();
        script(&[("ConvertArgument", 0)]);
        let result = unsafe { forward(&services(), 2) };
        assert_eq!(result, Ok(()));
        assert_eq!(
            calls(),
            vec![("IsDataDirectory", 1), ("ConvertArgument", 1), ("ReportInvalid", 1)]
        );
    }

    #[test]
    fn forward_appends_separator_before_argument_when_command_line_nonempty() {
        reset();
        script(&[("ConvertArgument", 1), ("CommandLineLength", 1)]);
        let result = unsafe { forward(&services(), 2) };
        assert_eq!(result, Ok(()));
        assert_eq!(
            calls(),
            vec![
                ("IsDataDirectory", 1),
                ("ConvertArgument", 1),
                ("CommandLineLength", 1),
                ("AppendSeparator", 1),
                ("AppendArgument", 1),
            ]
        );
    }

    #[test]
    fn forward_skips_separator_when_command_line_is_still_empty() {
        reset();
        script(&[("ConvertArgument", 1), ("CommandLineLength", 0)]);
        let result = unsafe { forward(&services(), 2) };
        assert_eq!(result, Ok(()));
        assert_eq!(
            calls(),
            vec![
                ("IsDataDirectory", 1),
                ("ConvertArgument", 1),
                ("CommandLineLength", 1),
                ("AppendArgument", 1),
            ]
        );
    }

    #[test]
    fn forward_visits_every_argument_from_index_one_through_argc_minus_one() {
        reset();
        script(&[("ConvertArgument", 1), ("CommandLineLength", 1)]);
        let result = unsafe { forward(&services(), 4) };
        assert_eq!(result, Ok(()));
        let indices: Vec<i32> = calls()
            .into_iter()
            .filter(|(name, _)| *name == "IsDataDirectory")
            .map(|(_, index)| index)
            .collect();
        assert_eq!(indices, vec![1, 2, 3]);
    }

    #[test]
    fn forward_visits_no_arguments_when_argc_is_one() {
        reset();
        let result = unsafe { forward(&services(), 1) };
        assert_eq!(result, Ok(()));
        assert!(calls().is_empty());
    }

    #[test]
    fn forward_propagates_first_callback_failure_immediately() {
        reset();
        fail("ConvertArgument");
        let result = unsafe { forward(&services(), 2) };
        assert_eq!(result, Err(7));
        assert_eq!(calls(), vec![("IsDataDirectory", 1), ("ConvertArgument", 1)]);
    }

    #[test]
    fn status_buffer_short_circuits_without_a_raw_buffer() {
        reset();
        let result = unsafe { status_buffer(&services(), false) };
        assert_eq!(result, Ok(false));
        assert!(calls().is_empty());
    }

    #[test]
    fn status_buffer_converts_when_raw_buffer_present_and_sdl_status_nonzero() {
        reset();
        script(&[("ConvertStatus", 1)]);
        let result = unsafe { status_buffer(&services(), true) };
        assert_eq!(result, Ok(true));
        assert_eq!(calls(), vec![("ConvertStatus", 0)]);
    }

    #[test]
    fn status_buffer_false_when_raw_buffer_present_but_sdl_status_zero() {
        reset();
        let result = unsafe { status_buffer(&services(), true) };
        assert_eq!(result, Ok(false));
        assert_eq!(calls(), vec![("ConvertStatus", 0)]);
    }

    #[test]
    fn status_buffer_propagates_callback_failure() {
        reset();
        fail("ConvertStatus");
        let result = unsafe { status_buffer(&services(), true) };
        assert_eq!(result, Err(7));
    }

    /// A single `forward()` call where argument indices take different
    /// branches: index 1 is a data directory (skipped before conversion is
    /// even queried), index 2 fails conversion (reported, not appended),
    /// index 3 converts with the command line still empty (no separator),
    /// and index 4 converts after it -- so it gets one. Exercises the
    /// separator logic depending on per-call native state, not a single
    /// script value applied uniformly to every index.
    #[test]
    fn forward_handles_a_mixed_argv_sequence_with_skip_fail_and_growing_command_line() {
        reset();
        script_at(&[
            ("IsDataDirectory", 1, 1),
            ("ConvertArgument", 2, 0),
            ("ConvertArgument", 3, 1),
            ("CommandLineLength", 3, 0),
            ("ConvertArgument", 4, 1),
            ("CommandLineLength", 4, 1),
        ]);
        let result = unsafe { forward(&services(), 5) };
        assert_eq!(result, Ok(()));
        assert_eq!(
            calls(),
            vec![
                ("IsDataDirectory", 1),
                ("IsDataDirectory", 2),
                ("ConvertArgument", 2),
                ("ReportInvalid", 2),
                ("IsDataDirectory", 3),
                ("ConvertArgument", 3),
                ("CommandLineLength", 3),
                ("AppendArgument", 3),
                ("IsDataDirectory", 4),
                ("ConvertArgument", 4),
                ("CommandLineLength", 4),
                ("AppendSeparator", 4),
                ("AppendArgument", 4),
            ]
        );
    }
}
