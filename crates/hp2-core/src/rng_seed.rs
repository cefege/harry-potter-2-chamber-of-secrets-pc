//! Strict pre-appInit diagnostic seed policy, independent of native startup state.

/// cbindgen:prefix-with-name
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum DiagnosticRngSeedBootstrap {
    Absent = 0,
    Accepted = 1,
    Invalid = 2,
}

/// Accept only nonempty ASCII decimal within the caller's `RAND_MAX` domain.
/// Leading zeroes are significant input but do not change the numeric value.
///
/// # Safety
/// `text` is readable through the first invalid/overflowing byte or its NUL
/// terminator. Parsing stops immediately on invalid input, without a length scan.
pub unsafe fn parse(mut text: *const u8, maximum: u32) -> Option<u32> {
    if text.is_null() || unsafe { text.read() } == 0 {
        return None;
    }
    let cutoff = maximum / 10;
    let final_digit = maximum % 10;
    let mut value = 0u32;
    loop {
        // SAFETY: the caller provides readable input through termination/error.
        let byte = unsafe { text.read() };
        if byte == 0 {
            return Some(value);
        }
        if !byte.is_ascii_digit() {
            return None;
        }
        let digit = u32::from(byte - b'0');
        if value > cutoff || (value == cutoff && digit > final_digit) {
            return None;
        }
        value = value * 10 + digit;
        // SAFETY: input remains readable until NUL or the first rejected byte.
        text = unsafe { text.add(1) };
    }
}

/// Both zero-modulus diagnostic endpoints use the calibrated stream seeded by one.
pub fn effective(seed: u32, maximum: u32) -> u32 {
    if seed == 0 || seed == maximum {
        1
    } else {
        seed
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;
    use crate::rng_private;

    const RAND_MAX: u32 = 0x7fff_ffff;

    fn terminated(text: &[u8]) -> Vec<u8> {
        let mut terminated = text.to_vec();
        terminated.push(0);
        terminated
    }

    fn parse_text(text: &[u8], maximum: u32) -> Option<u32> {
        let text = terminated(text);
        // SAFETY: `text` is NUL-terminated and outlives the call.
        unsafe { parse(text.as_ptr(), maximum) }
    }

    /// Mirrors `hp2_rs_core_rng_seed_parse`'s output contract: the caller's
    /// slot changes only when parsing succeeds.
    fn parse_into(text: &[u8], maximum: u32, output: &mut u32) -> bool {
        match parse_text(text, maximum) {
            Some(value) => {
                *output = value;
                true
            }
            None => false,
        }
    }

    /// Test-local transcription of `hp2_rs_core_rng_seed_prepare`'s policy
    /// (`crates/hp2-ffi/src/core_rng_seed.rs`): absent input takes precedence
    /// over missing outputs, invalid input leaves both outputs untouched, and
    /// accepted input publishes requested then effective.
    ///
    /// # Safety
    /// Non-null `text` is NUL-terminated; non-null outputs are writable `u32`
    /// slots that may alias each other or the input.
    unsafe fn prepare(
        text: *const u8,
        maximum: u32,
        requested: *mut u32,
        effective_seed: *mut u32,
    ) -> DiagnosticRngSeedBootstrap {
        if text.is_null() || unsafe { text.read() } == 0 {
            return DiagnosticRngSeedBootstrap::Absent;
        }
        if requested.is_null() || effective_seed.is_null() {
            return DiagnosticRngSeedBootstrap::Invalid;
        }
        // SAFETY: the caller provides NUL-terminated input.
        let Some(value) = (unsafe { parse(text, maximum) }) else {
            return DiagnosticRngSeedBootstrap::Invalid;
        };
        // SAFETY: outputs are writable; sequential raw writes preserve aliasing.
        unsafe {
            requested.write(value);
            effective_seed.write(effective(value, maximum));
        }
        DiagnosticRngSeedBootstrap::Accepted
    }

    fn prepare_text(
        text: Option<&[u8]>,
        maximum: u32,
        requested: &mut u32,
        effective_seed: &mut u32,
    ) -> DiagnosticRngSeedBootstrap {
        let text = text.map(terminated);
        let pointer = text.as_ref().map_or(std::ptr::null(), |text| text.as_ptr());
        // SAFETY: `pointer` is null or NUL-terminated; both outputs are live.
        unsafe { prepare(pointer, maximum, requested, effective_seed) }
    }

    /// Guards the `#[ignore]`d helpers so a stray `--ignored` run never seeds,
    /// exits, or writes trace state from the test harness process.
    fn contract_child() -> bool {
        std::env::var_os("HP2_RNG_SEED_CONTRACT_CHILD").is_some()
    }

    fn run_bootstrap_child(
        name: &str,
        environment: &[(&str, &str)],
        removed: &[&str],
    ) -> std::process::Output {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([name, "--exact", "--ignored", "--test-threads=1"]);
        for (key, value) in environment {
            command.env(key, value);
        }
        for key in removed {
            command.env_remove(key);
        }
        command.env("HP2_RNG_SEED_CONTRACT_CHILD", "1");
        command.output().unwrap()
    }

    #[test]
    fn parse_accepts_diagnostic_seed_values() {
        assert_eq!(parse_text(b"0", RAND_MAX), Some(0));
        assert_eq!(parse_text(b"00042", RAND_MAX), Some(42));
        assert_eq!(
            parse_text(RAND_MAX.to_string().as_bytes(), RAND_MAX),
            Some(RAND_MAX)
        );
    }

    #[test]
    fn parse_rejects_invalid_seed_values_without_writing_output() {
        let mut output = 99;
        let accepted = match unsafe { parse(std::ptr::null(), RAND_MAX) } {
            Some(value) => {
                output = value;
                true
            }
            None => false,
        };
        assert!(!accepted, "NULL");
        assert_eq!(output, 99, "invalid parse modified output");

        let too_large = (u64::from(RAND_MAX) + 1).to_string();
        for text in [
            &b""[..],
            b"-1",
            b"+1",
            b" 1",
            b"1 ",
            b"0x1",
            b"1.0",
            too_large.as_bytes(),
            b"999999999999999999999999999999999999",
        ] {
            output = 99;
            assert!(
                !parse_into(text, RAND_MAX, &mut output),
                "{}",
                String::from_utf8_lossy(text)
            );
            assert_eq!(output, 99, "invalid parse modified output");
        }
    }

    #[test]
    fn parse_uses_callers_libc_seed_domain() {
        let mut domain = 99;
        assert!(
            !parse_into(b"32768", 32767, &mut domain) && domain == 99,
            "parser ignored caller's smaller libc seed domain"
        );
        assert!(
            parse_into(b"4294967295", u32::MAX, &mut domain) && domain == u32::MAX,
            "parser overflowed the full unsigned seed domain"
        );
    }

    #[test]
    fn parse_consumes_overlapping_input_before_writing() {
        let mut overlapping = [0u8; 4];
        overlapping[..3].copy_from_slice(b"42\0");
        // SAFETY: the buffer is NUL-terminated at index 2.
        let parsed = unsafe { parse(overlapping.as_ptr(), RAND_MAX) };
        assert_eq!(
            parsed,
            Some(42),
            "parse overwrote overlapping input before consuming it"
        );
        // The native contract writes the result back over the input bytes.
        if let Some(value) = parsed {
            unsafe {
                overlapping
                    .as_mut_ptr()
                    .cast::<u32>()
                    .write_unaligned(value)
            };
        }
        let stored = unsafe { overlapping.as_ptr().cast::<u32>().read_unaligned() };
        assert_eq!(
            stored, 42,
            "parse overwrote overlapping input before consuming it"
        );
    }

    #[test]
    fn prepare_reports_absent_seed() {
        for text in [None, Some(&b""[..])] {
            let mut requested = 71;
            let mut effective_seed = 73;
            assert_eq!(
                prepare_text(text, RAND_MAX, &mut requested, &mut effective_seed),
                DiagnosticRngSeedBootstrap::Absent,
                "{text:?}"
            );
            assert!(
                requested == 71 && effective_seed == 73,
                "absent bootstrap modified outputs"
            );
        }
    }

    #[test]
    fn prepare_accepts_seed_values() {
        let maximum = RAND_MAX.to_string().into_bytes();
        for (text, expected_requested, expected_effective) in [
            (b"0".to_vec(), 0, 1),
            (b"00042".to_vec(), 42, 42),
            (maximum, RAND_MAX, 1),
        ] {
            let mut requested = 0;
            let mut effective_seed = 0;
            assert_eq!(
                prepare_text(Some(&text), RAND_MAX, &mut requested, &mut effective_seed),
                DiagnosticRngSeedBootstrap::Accepted,
                "{}",
                String::from_utf8_lossy(&text)
            );
            assert_eq!(
                requested, expected_requested,
                "bootstrap requested seed changed"
            );
            assert_eq!(
                effective_seed, expected_effective,
                "bootstrap effective seed changed"
            );
        }
    }

    #[test]
    fn prepare_rejects_invalid_seed_values() {
        let too_large = (u64::from(RAND_MAX) + 1).to_string();
        for text in [
            &b"-1"[..],
            b"+1",
            b" 1",
            b"1 ",
            b"0x1",
            b"1.0",
            too_large.as_bytes(),
            b"999999999999999999999999999999999999",
        ] {
            let mut requested = 71;
            let mut effective_seed = 73;
            assert_eq!(
                prepare_text(Some(text), RAND_MAX, &mut requested, &mut effective_seed),
                DiagnosticRngSeedBootstrap::Invalid,
                "{}",
                String::from_utf8_lossy(text)
            );
            assert!(
                requested == 71 && effective_seed == 73,
                "invalid bootstrap modified outputs"
            );
        }
    }

    #[test]
    fn prepare_rejects_missing_outputs() {
        assert_eq!(
            // SAFETY: the literal is NUL-terminated; both outputs are null.
            unsafe {
                prepare(
                    b"1\0".as_ptr(),
                    RAND_MAX,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            DiagnosticRngSeedBootstrap::Invalid,
            "bootstrap accepted null outputs"
        );
        assert_eq!(
            // SAFETY: null input with null outputs is the absent precedence case.
            unsafe {
                prepare(
                    std::ptr::null(),
                    RAND_MAX,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            DiagnosticRngSeedBootstrap::Absent,
            "absent bootstrap inspected missing outputs"
        );

        let mut requested = 71;
        assert_eq!(
            // SAFETY: the literal is NUL-terminated; `requested` is live.
            unsafe {
                prepare(
                    b"1\0".as_ptr(),
                    RAND_MAX,
                    &mut requested,
                    std::ptr::null_mut(),
                )
            },
            DiagnosticRngSeedBootstrap::Invalid
        );
        assert_eq!(
            requested, 71,
            "missing effective output changed requested seed"
        );

        let mut effective_seed = 73;
        assert_eq!(
            // SAFETY: the literal is NUL-terminated; `effective_seed` is live.
            unsafe {
                prepare(
                    b"1\0".as_ptr(),
                    RAND_MAX,
                    std::ptr::null_mut(),
                    &mut effective_seed,
                )
            },
            DiagnosticRngSeedBootstrap::Invalid
        );
        assert_eq!(
            effective_seed, 73,
            "missing requested output changed effective seed"
        );
    }

    #[test]
    fn prepare_alias_publishes_requested_then_effective() {
        let mut aliased = 71u32;
        let slot = std::ptr::addr_of_mut!(aliased);
        // SAFETY: both output arguments are the same live, writable slot.
        assert_eq!(
            unsafe { prepare(b"0\0".as_ptr(), RAND_MAX, slot, slot) },
            DiagnosticRngSeedBootstrap::Accepted,
            "aliased bootstrap outputs lost effective-seed publication order"
        );
        assert_eq!(
            aliased, 1,
            "aliased bootstrap outputs lost effective-seed publication order"
        );
    }

    #[test]
    fn effective_normalizes_endpoint_seeds() {
        assert_eq!(effective(0, RAND_MAX), 1, "zero must normalize to one");
        assert_eq!(
            effective(RAND_MAX, RAND_MAX),
            1,
            "RAND_MAX must normalize to one"
        );
        assert_eq!(effective(42, RAND_MAX), 42, "ordinary seed changed");
    }

    #[test]
    #[ignore = "fresh-process helper invoked by the rng seed bootstrap contracts"]
    fn bootstrap_default_context_child() {
        if !contract_child() {
            return;
        }
        rng_private::srand(42);
        let expected = rng_private::rand();
        rng_private::srand(42);
        // SAFETY: single-threaded fresh process; HP2_RNG_SEED is absent.
        unsafe { crate::unix_rng_bootstrap::bootstrap_diagnostic_rng_seed(RAND_MAX) };
        assert_eq!(
            rng_private::rand(),
            expected,
            "Context consumed or changed the seeded stream"
        );
    }

    #[test]
    #[ignore = "fresh-process helper invoked by the rng seed bootstrap contracts"]
    fn bootstrap_diagnostic_context_child() {
        if !contract_child() {
            return;
        }
        rng_private::srand(1);
        let expected = rng_private::rand();
        rng_private::srand(1);
        // SAFETY: single-threaded fresh process; HP2_RNG_SEED is "0".
        unsafe { crate::unix_rng_bootstrap::bootstrap_diagnostic_rng_seed(RAND_MAX) };
        assert_eq!(
            rng_private::rand(),
            expected,
            "Context consumed or changed the seeded stream"
        );
    }

    #[test]
    #[ignore = "fresh-process helper invoked by the rng seed bootstrap contracts"]
    fn bootstrap_invalid_context_child() {
        if !contract_child() {
            return;
        }
        // SAFETY: single-threaded fresh process; HP2_RNG_SEED is "invalid",
        // which exits one after its diagnostic.
        unsafe { crate::unix_rng_bootstrap::bootstrap_diagnostic_rng_seed(RAND_MAX) };
    }

    #[test]
    #[ignore = "fresh-process helper invoked by the rng seed bootstrap contracts"]
    fn bootstrap_late_trace_context_child() {
        if !contract_child() {
            return;
        }
        let artifact_directory =
            std::env::var("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR is required");
        assert!(
            !artifact_directory.is_empty(),
            "HP2_ARTIFACT_DIR is required"
        );
        let trace_path =
            std::path::Path::new(&artifact_directory).join("context-post-seam-rng-trace.json");
        // SAFETY: single-threaded fresh process, before any concurrent env read.
        unsafe { std::env::set_var("HP2_RNG_SEED", "42") };
        // SAFETY: single-threaded fresh process.
        unsafe { crate::unix_rng_bootstrap::bootstrap_diagnostic_rng_seed(RAND_MAX) };
        // SAFETY: the trace environment is armed only after the bootstrap returns.
        unsafe { std::env::set_var("HP2_RNG_TRACE", &trace_path) };
        crate::ansi_trace::with_process_state(crate::ansi_trace::direct_rand_value);
        crate::ansi_trace::flush_process_state();
        assert!(
            std::fs::File::open(&trace_path).is_ok(),
            "Context initialized tracing before trace environment existed"
        );
    }

    #[test]
    fn bootstrap_default_context_preserves_seeded_stream() {
        let output = run_bootstrap_child(
            "rng_seed::tests::bootstrap_default_context_child",
            &[],
            &["HP2_RNG_SEED", "HP2_RNG_TRACE"],
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "default context child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn bootstrap_diagnostic_zero_seeds_one() {
        let output = run_bootstrap_child(
            "rng_seed::tests::bootstrap_diagnostic_context_child",
            &[("HP2_RNG_SEED", "0")],
            &["HP2_RNG_TRACE"],
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "diagnostic context child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn bootstrap_invalid_seed_fails_before_app_init() {
        let output = run_bootstrap_child(
            "rng_seed::tests::bootstrap_invalid_context_child",
            &[("HP2_RNG_SEED", "invalid")],
            &["HP2_RNG_TRACE"],
        );
        assert_eq!(
            output.status.code(),
            Some(1),
            "invalid seed did not exit one"
        );
        let expected = format!(
            "Invalid HP2_RNG_SEED: expected an unsigned decimal value from 0 to {RAND_MAX}\n"
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            expected,
            "invalid seed stderr changed"
        );
    }

    #[test]
    fn bootstrap_late_trace_arms_after_context() {
        let directory =
            std::env::temp_dir().join(format!("hp2-rng-context-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let output = run_bootstrap_child(
            "rng_seed::tests::bootstrap_late_trace_context_child",
            &[("HP2_ARTIFACT_DIR", directory.to_str().unwrap())],
            &["HP2_RNG_SEED", "HP2_RNG_TRACE", "HP2_RNG_TRACE_LIMIT"],
        );
        let trace = directory.join("context-post-seam-rng-trace.json");
        let trace_exists = trace.is_file();
        let _ = std::fs::remove_dir_all(&directory);
        assert_eq!(
            output.status.code(),
            Some(0),
            "late trace child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            trace_exists,
            "Context initialized tracing before trace environment existed"
        );
    }
}
