//! Rust port of `AGameInfo::ParseKillMessage`
//! (`HarryPotter2/Unreal/Engine/Src/UnScript.cpp`, `ScriptGameInfoParseKillMessage`).
//!
//! `NetworkNumber` stays a C++ leaf (`ScriptGameInfoNetworkNumber` in `UnScript.cpp`):
//! `G->XLevel->NetDriver->LowLevelGetNetworkNumber()` is a virtual call on a still-C++
//! `UNetDriver`, so it cannot move to Rust while `UNetDriver` keeps its C++ vtable.
//!
//! The original builds `Message` by substituting `%k`/`%o`/`%w` with the killer/victim/
//! weapon name, left to right, each stage re-reading the previous stage's `Message` into
//! `Temp`. There is no `else`: a token that is not found leaves `Message` exactly as the
//! previous stage left it, not cleared. For the first (`%k`) stage specifically,
//! `Message` starts as a default-constructed empty `FString`, so a missing `%k` leaves
//! it empty; because `Temp = Message` then runs unconditionally, every later stage's
//! `InStr` searches an empty `Temp` and also misses, so the whole result stays empty. A
//! missing `%o` or `%w` *after* a successful earlier stage has no such effect: `Message`
//! simply keeps the prior stage's substitution.
//!
//! Three native-FString subtleties this keeps exactly:
//! - `InStr`/`FString::operator+=` are `wcsstr`/`wcscpy`-based: both only ever see a
//!   NUL-terminated prefix, so a search or an appended name stops at the first embedded
//!   NUL in that specific operand ([`nul_prefix`]).
//! - `Left(Offset)` never meets an embedded NUL here: `Offset` is `InStr`'s match
//!   position, strictly before `Temp`'s first NUL, so `Temp[..Offset]` is NUL-free by
//!   construction.
//! - `Right(Temp.Len() - Offset - 2)` is *not* a fixed-count copy: `FString::Right`
//!   (`Hp2StringValue` op 2) always resolves to a NUL-terminated-prefix copy from its
//!   start index (`string_storage::range_value`'s `count = None` branch), independent
//!   of the requested count. So the tail after a matched token is also NUL-prefix
//!   bounded, not the template's full raw remainder ([`nul_prefix`] again).

use crate::params::TChar;

/// The NUL-terminated prefix of `units` (`wcsstr`/`wcscpy` semantics: the scan stops at
/// the first embedded NUL rather than reading the caller's full declared length).
fn nul_prefix(units: &[TChar]) -> &[TChar] {
    let end = units.iter().position(|&c| c == 0).unwrap_or(units.len());
    &units[..end]
}

/// `Temp.InStr(needle)` for a fixed two-character `needle` ("%k"/"%o"/"%w"): the offset
/// of its first occurrence, or `None` for `Offset == -1`. The scan is bounded by `Temp`'s
/// own NUL-terminated prefix ([`nul_prefix`]), matching `appStrstr`/`wcsstr`.
fn find_token(haystack: &[TChar], needle: [TChar; 2]) -> Option<usize> {
    let scanned = nul_prefix(haystack);
    if scanned.len() < 2 {
        return None;
    }
    (0..=scanned.len() - 2).find(|&i| haystack[i] == needle[0] && haystack[i + 1] == needle[1])
}

/// One `if (Offset != -1) { Message = Temp.Left(Offset) + Name + Temp.Right(Temp.Len() -
/// Offset - 2); }` stage. `message` is left untouched on a miss (`Offset == -1`), matching
/// the original: the caller still unconditionally copies `message` into the next stage's
/// `Temp` afterward.
fn substitute(temp: &[TChar], needle: [TChar; 2], replacement: &[TChar], message: &mut Vec<TChar>) {
    let Some(offset) = find_token(temp, needle) else {
        return;
    };
    // `offset + 2 <= nul_prefix(temp).len() <= temp.len()`, so this slice never panics.
    let tail = nul_prefix(&temp[offset + 2..]);
    let mut next = Vec::with_capacity(offset + replacement.len() + tail.len());
    next.extend_from_slice(&temp[..offset]);
    next.extend_from_slice(nul_prefix(replacement));
    next.extend_from_slice(tail);
    *message = next;
}

/// `AGameInfo::ParseKillMessage`: substitutes `%k`, then `%o`, then `%w` in
/// `kill_message` with `killer_name`/`victim_name`/`weapon_name` and returns the result
/// (without a NUL terminator; the caller adds one for the native FString result).
///
/// Each input is the FString's full declared content (its `Len()` units, not a
/// NUL-terminated read): callers pass whatever `vm_string::view`-style reads already
/// produced. Internally this matches the original's per-operand truncation exactly:
/// `InStr`/`+=`/`Right`'s tail all stop at that operand's own first embedded NUL
/// ([`nul_prefix`]), while `Left`'s prefix is NUL-free by construction (see the module
/// doc).
pub fn parse_kill_message(
    killer_name: &[TChar],
    victim_name: &[TChar],
    weapon_name: &[TChar],
    kill_message: &[TChar],
) -> Vec<TChar> {
    const K: [TChar; 2] = [b'%' as TChar, b'k' as TChar];
    const O: [TChar; 2] = [b'%' as TChar, b'o' as TChar];
    const W: [TChar; 2] = [b'%' as TChar, b'w' as TChar];

    let mut message: Vec<TChar> = Vec::new();
    let mut temp: Vec<TChar> = kill_message.to_vec();

    substitute(&temp, K, killer_name, &mut message);
    temp = message.clone();

    substitute(&temp, O, victim_name, &mut message);
    temp = message.clone();

    substitute(&temp, W, weapon_name, &mut message);

    message
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(s: &str) -> Vec<TChar> {
        s.chars().map(|c| c as TChar).collect()
    }

    #[test]
    fn all_three_tokens_substitute_left_to_right() {
        let result = parse_kill_message(
            &units("Alice"),
            &units("Bob"),
            &units("Wand"),
            &units("%k killed %o with %w"),
        );
        assert_eq!(result, units("Alice killed Bob with Wand"));
    }

    #[test]
    fn missing_leading_token_collapses_the_whole_result_to_empty() {
        // `%k` is absent: `Message` stays at its default-empty value, and the
        // unconditional `Temp = Message;` then carries that empty value into the
        // `%o`/`%w` stages too, exactly as the original's defect reproduces.
        let result = parse_kill_message(
            &units("Alice"),
            &units("Bob"),
            &units("Wand"),
            &units("killed %o with %w"),
        );
        assert_eq!(result, Vec::<TChar>::new());
    }

    #[test]
    fn missing_middle_token_keeps_the_leading_substitution() {
        let result = parse_kill_message(
            &units("Alice"),
            &units("Bob"),
            &units("Wand"),
            &units("%k killed with %w"),
        );
        assert_eq!(result, units("Alice killed with Wand"));
    }

    #[test]
    fn no_tokens_at_all_yields_empty() {
        let result = parse_kill_message(
            &units("Alice"),
            &units("Bob"),
            &units("Wand"),
            &units("a plain string"),
        );
        assert_eq!(result, Vec::<TChar>::new());
    }

    #[test]
    fn empty_kill_message_yields_empty() {
        let result = parse_kill_message(&units("Alice"), &units("Bob"), &units("Wand"), &[]);
        assert_eq!(result, Vec::<TChar>::new());
    }

    #[test]
    fn replacement_name_stops_at_its_own_embedded_nul() {
        // `Message += *(const FString*)KillerName` is `wcscpy`-based: only the
        // NUL-terminated prefix of `KillerName` is appended, even though the FString's
        // declared length (what callers pass here) may run past that NUL.
        let mut killer = units("Ali");
        killer.push(0);
        killer.extend(units("ce"));
        let result = parse_kill_message(&killer, &units("Bob"), &units("Wand"), &units("%k wins"));
        assert_eq!(result, units("Ali wins"));
    }

    #[test]
    fn template_embedded_nul_after_the_token_truncates_the_right_tail() {
        // `Temp.Right(Temp.Len() - Offset - 2)` always resolves through
        // `string_storage::range_value`'s NUL-terminated-prefix branch, so an embedded
        // NUL in the template tail (after the matched token) stops the copy there;
        // nothing past that NUL survives into the result.
        let mut message = units("%k killed");
        message.push(0);
        message.extend(units("X"));
        let result = parse_kill_message(&units("Alice"), &units("Bob"), &units("Wand"), &message);
        assert_eq!(result, units("Alice killed"));
    }

    #[test]
    fn template_embedded_nul_before_the_token_hides_it_from_the_search() {
        // `InStr` is `wcsstr`-based: a NUL before `%k` stops the search, so the token
        // past it is never found, and (per the leading-token defect) the result is empty.
        let mut message = units("intro");
        message.push(0);
        message.extend(units("%k killed"));
        let result = parse_kill_message(&units("Alice"), &units("Bob"), &units("Wand"), &message);
        assert_eq!(result, Vec::<TChar>::new());
    }

    #[test]
    fn repeated_token_only_the_first_occurrence_substitutes() {
        let result = parse_kill_message(
            &units("Alice"),
            &units("Bob"),
            &units("Wand"),
            &units("%k %k killed %o"),
        );
        assert_eq!(result, units("Alice %k killed Bob"));
    }

    /// Cross-checks [`find_token`] against the real `hp2_core::string_storage::search`
    /// that `Hp2StringSearch`/`InStr` actually calls (no allocator needed: `search` is
    /// read-only), closing the one genuinely novel semantic this module depends on
    /// (NUL-terminated-prefix search) against production code rather than this
    /// module's own re-derivation of it.
    #[test]
    fn find_token_matches_the_real_string_storage_search() {
        use hp2_core::array_storage::Hp2ArraySlots;
        use hp2_core::string_storage;

        fn real_search(haystack: &[TChar], needle: [TChar; 2]) -> i32 {
            // FString storage convention: a NUL-terminated buffer, `num` includes the
            // terminator.
            let mut buf: Vec<TChar> = haystack.to_vec();
            buf.push(0);
            let mut data_ptr: *mut std::ffi::c_void = buf.as_mut_ptr().cast();
            let mut num = buf.len() as i32;
            let mut max = num;
            let slots = Hp2ArraySlots {
                data: &raw mut data_ptr,
                num: &raw mut num,
                max: &raw mut max,
            };
            let needle_buf: [TChar; 3] = [needle[0], needle[1], 0];
            unsafe { string_storage::search(slots, needle_buf.as_ptr(), false) }
        }

        const K: [TChar; 2] = [b'%' as TChar, b'k' as TChar];

        let mut haystacks: Vec<Vec<TChar>> = vec![
            units(""),
            units("no token"),
            units("%k at start"),
            units("ends with %k"),
            units("%k %k repeated"),
            units("x"),
            units("%"),
        ];
        // Embedded NUL before the token (hides it) and after the token (irrelevant).
        let mut before = units("intro");
        before.push(0);
        before.extend(units("%k tail"));
        haystacks.push(before);
        let mut after = units("%k killed");
        after.push(0);
        after.extend(units("tail"));
        haystacks.push(after);

        for haystack in &haystacks {
            let expected = real_search(haystack, K);
            let actual = find_token(haystack, K).map_or(-1, |i| i as i32);
            assert_eq!(
                actual, expected,
                "haystack {haystack:?} disagreed with the real string_storage::search"
            );
        }
    }
}
