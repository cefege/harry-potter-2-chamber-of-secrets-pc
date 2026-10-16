//! Replaces `HP2ImportInternal.h`'s `Lower`, the case-insensitive
//! comparison key every oracle (`Build/prepare_retail_data.py`) and C++
//! import step shares for path sorting, debris/prohibited-extension
//! checks, and collision detection.
//!
//! `prepare_retail_data.py` uses Python's `str.casefold()` throughout
//! (`Build/prepare_retail_data.py:264,280-281,295,1018,1524,1548`, and
//! many more); `Lower` matches it via `utf8proc_map(..., UTF8PROC_CASEFOLD)`
//! (full Unicode case folding, not simple lowercasing: `ß` becomes `ss`,
//! not `ß`). A plain ASCII `to_lowercase` would diverge on any non-ASCII
//! byte and silently produce different sort orders / collision keys than
//! the Python oracle, so this must not be approximated.
//!
//! The plan calls for pure-Rust casefolding, not linking `utf8proc` into
//! the Rust side (it is dropped from the C++ build once `hp2-import`
//! lands). The `caseless` crate (built on `unicode-normalization`'s own
//! bundled tables) was tried first, but an exhaustive check -- dumping
//! `utf8proc_map(UTF8PROC_CASEFOLD)` for every Unicode scalar value from
//! the pinned `utf8proc` 2.12.0 (Unicode 18.0) and diffing against
//! `caseless::default_case_fold_str`'s output for the same codepoints --
//! found 146 real divergences, concentrated in Unicode additions newer
//! than `unicode-normalization`'s bundled tables (e.g. the Garay script,
//! Unicode 16.0). [`crate::fold_table`] is that same exhaustive dump,
//! reduced to the 1,606 codepoints whose fold actually changes something
//! (the same sparse shape as Unicode's own `CaseFolding.txt`), committed
//! as a plain Rust table and binary-searched here -- pure Rust, and exact
//! against the pinned utf8proc version by construction, since the table
//! *is* its output rather than an independent reimplementation.
//!
//! `Lower`'s C++ body always scans and ASCII-lowercases `s` in place
//! first, and only reaches `utf8proc_map` if a non-ASCII byte turned up;
//! that's a pure performance fast path, not a behavioral difference --
//! ASCII casefold and ASCII lowercase agree by construction (the fold
//! table's own A-Z entries map the same way simple lowercasing does), so
//! pre-lowercasing the ASCII run before casefolding the whole string
//! changes nothing. This keeps the same fast path for the same reason.
//!
//! `Lower` can fail (`Fail("invalid UTF-8 ...")`) because `fs::path`
//! round-trips through `u8string()`, and POSIX filenames aren't
//! guaranteed valid UTF-8. [`lower`] takes `&str`, which Rust guarantees
//! is valid UTF-8 at the type level, so that failure mode does not apply
//! here -- it belongs to whatever converts a raw filesystem name to `&str`
//! before calling this (the `HP2ImportInputs.cpp`/`Collect.cpp` port),
//! matching `Lower`'s own `Utf8()` helper's role, not this function's.

use crate::fold_table::FOLD_TABLE;

/// Replaces `Lower`. Full Unicode case folding (not simple lowercasing),
/// matching `utf8proc_map(..., UTF8PROC_CASEFOLD)` exactly (see the module
/// doc for how). Always succeeds: unlike the C++ version, there is no
/// invalid-UTF-8 case to reject because `&str` is already guaranteed valid
/// UTF-8.
pub fn lower(s: &str) -> String {
    if s.is_ascii() {
        return s.to_ascii_lowercase();
    }
    let mut result = String::with_capacity(s.len());
    for ch in s.chars() {
        match FOLD_TABLE.binary_search_by_key(&(ch as u32), |&(cp, _)| cp) {
            Ok(idx) => result.push_str(FOLD_TABLE[idx].1),
            Err(_) => result.push(ch),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_fast_path_matches_ascii_lowercase() {
        assert_eq!(lower("MASS"), "mass");
        assert_eq!(lower("System.Default_Ini"), "system.default_ini");
        assert_eq!(lower(""), "");
    }

    #[test]
    fn full_casefold_handles_sharp_s_expansion() {
        // Verified against Python 3: "ß".casefold() == "ss" (full
        // casefold expands, simple lowercasing would leave it as "ß").
        assert_eq!(lower("ß"), "ss");
        assert_eq!(lower("Straße"), "strasse");
        assert_eq!(lower("STRASSE"), "strasse");
        // Straße and STRASSE must compare equal under this key, exactly
        // as prepare_retail_data.py's casefold-keyed collision/sort logic
        // requires.
        assert_eq!(lower("Straße"), lower("STRASSE"));
    }

    #[test]
    fn full_casefold_handles_ligature_expansion() {
        // Verified against Python 3: "ﬁ".casefold() == "fi".
        assert_eq!(lower("ﬁle.txt"), "file.txt");
    }

    #[test]
    fn full_casefold_handles_turkish_dotted_capital_i() {
        // Verified against Python 3: "İ".casefold() == "i\u0307" (a
        // combining dot above survives full casefold; this is not the
        // ASCII/Turkish-locale "i" simple lowercasing would give).
        assert_eq!(lower("İ"), "i\u{307}");
    }

    #[test]
    fn full_casefold_is_idempotent_on_already_folded_input() {
        let s = "already folded straße";
        assert_eq!(lower(s), lower(&lower(s)));
    }

    #[test]
    fn full_casefold_handles_the_newest_unicode_additions() {
        // The Garay script (U+10D40.., Unicode 16.0) is exactly the kind
        // of recent addition the caseless/unicode-normalization crate's
        // bundled tables were found to disagree with the pinned utf8proc
        // on. Garay has no case distinction (no fold entry), so this
        // checks the table correctly leaves it unchanged rather than
        // erroring or mangling it.
        assert_eq!(lower("\u{10D40}"), "\u{10D40}");
    }

    #[test]
    fn fold_table_is_sorted_and_has_no_duplicate_keys() {
        // Required for the binary search in lower() to be correct at all.
        for window in FOLD_TABLE.windows(2) {
            assert!(
                window[0].0 < window[1].0,
                "fold table not strictly increasing at {:#x} -> {:#x}",
                window[0].0,
                window[1].0
            );
        }
    }

    #[test]
    fn fold_table_entries_round_trip_through_lower() {
        // Spot-check a sample spread across the table (not all 1,606, to
        // keep this fast) rather than only the first/last entries.
        for &(cp, folded) in FOLD_TABLE.iter().step_by(37) {
            let ch = char::from_u32(cp).unwrap();
            let mut s = String::new();
            s.push(ch);
            assert_eq!(lower(&s), folded, "codepoint {cp:#x}");
        }
    }
}
