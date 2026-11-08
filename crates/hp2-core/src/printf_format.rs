//! Pure format-string parsing and bounds-checked buffer-append logic behind
//! `appVswprintfCompat` (`UnGnuG.h`). That function hand-rolls a printf
//! interpreter so wide strings/chars never cross POSIX's locale-dependent
//! `vswprintf`; numeric conversions alone are delegated to the real libc
//! `std::vswprintf`, one directive at a time through a `va_copy` snapshot.
//!
//! `va_arg` consumption and the `std::vswprintf` call cannot move here: this
//! project's FFI style is `extern "C"` leaves with plain argument types, and
//! `va_list` cannot cross that boundary (Rust's C-variadic/`VaList` support
//! is nightly-only, which this project does not use). So those two things
//! stay textually in the C++ driver. Everything else — recognizing flags,
//! width, precision, length modifiers and the conversion character, building
//! the per-directive `vswprintf` sub-format string, and every bounds-checked
//! output append — is pure computation and lives here instead.
//!
//! The driver calls [`parse_directive`] once per `%` directive, right after
//! consuming the `%` itself. When a `*` width or precision needs an argument,
//! the function stops and reports [`DirectiveAction::NeedWidthArg`] /
//! [`DirectiveAction::NeedPrecisionArg`]; the driver performs the single
//! `va_arg(Args, int)` call itself and resumes parsing by calling back in
//! with `stage` advanced (`1` after a width arg, `2` after a precision arg)
//! and the fetched value in `arg_value`. This keeps every `va_arg` call
//! textually in C++ while the surrounding state machine — which characters
//! are flags, where width/precision end, how a `*`-resolved width/precision
//! renders as decimal digits — stays here, matching the original
//! `appVswprintfPut`/`AppendText`/`AppendInt` bounds semantics bit for bit:
//! a literal flag/width/precision/length-modifier/conversion character always
//! fits (the lenient `PutSeg` truncation the original used), but a
//! `*`-resolved width or precision can overflow the segment buffer and fails
//! the whole directive, exactly like the original's direct
//! `appVswprintfAppendInt(Segment, ...)` call (not `PutSeg`).

use crate::ansi::Tchar;

/// `AppVswLength` (`UnGnuG.h`): the length modifier recognized for a
/// directive. Discriminants match `AppVswLength`'s declaration order so the
/// C++ driver can `static_cast` the raw `u32` straight back to its own enum.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Length {
    None = 0,
    H = 1,
    Hh = 2,
    L = 3,
    Ll = 4,
    Z = 5,
    J = 6,
    T = 7,
    BigL = 8,
}

/// What the driver must do next to continue a directive started by
/// [`parse_directive`].
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DirectiveAction {
    /// Call `va_arg(Args, int)` for a `*` width, then resume with `stage = 1`
    /// and the fetched value in `arg_value`.
    NeedWidthArg = 0,
    /// Call `va_arg(Args, int)` for a `*` precision, then resume with
    /// `stage = 2` and the fetched value in `arg_value`.
    NeedPrecisionArg = 1,
    /// The directive is fully parsed: `length_out`/`conversion_out` are set,
    /// `segment` is NUL-terminated, and `*in_out` addresses the character
    /// right after the conversion.
    Done = 2,
    /// `Format` ended without a conversion character, or a `*`-resolved
    /// width/precision overran the segment buffer. The whole
    /// `appVswprintfCompat` call must fail: `Dest[0] = 0; return -1;`.
    Overflow = 3,
}

/// Bounds-checked single-character append; mirrors `appVswprintfPut`.
/// Returns `false`, writing nothing, once `*out + 1 >= count`.
///
/// # Safety
/// `dest` must be writable for `count` `Tchar` elements and `*out <= count`.
pub unsafe fn put(dest: *mut Tchar, count: usize, out: &mut usize, ch: Tchar) -> bool {
    if *out + 1 >= count {
        return false;
    }
    unsafe {
        *dest.add(*out) = ch;
    }
    *out += 1;
    true
}

/// Bounds-checked NUL-terminated wide-text append; mirrors
/// `appVswprintfAppendText`.
///
/// # Safety
/// `dest` must be writable for `count`; `text` must be NUL-terminated and
/// readable.
pub unsafe fn append_text(dest: *mut Tchar, count: usize, out: &mut usize, text: *const Tchar) -> bool {
    let mut index = 0usize;
    loop {
        let ch = unsafe { *text.add(index) };
        if ch == 0 {
            return true;
        }
        if !unsafe { put(dest, count, out, ch) } {
            return false;
        }
        index += 1;
    }
}

/// Bounds-checked fixed-length wide-text append (no NUL dependency); used to
/// copy a literal run out of `Format` directly, between directives.
///
/// # Safety
/// `dest` must be writable for `count`; `text` must be readable for `len`
/// elements.
pub unsafe fn append_text_n(dest: *mut Tchar, count: usize, out: &mut usize, text: *const Tchar, len: usize) -> bool {
    for index in 0..len {
        let ch = unsafe { *text.add(index) };
        if !unsafe { put(dest, count, out, ch) } {
            return false;
        }
    }
    true
}

/// Bounds-checked narrow-text append with the legacy `%hs` byte widening
/// (`static_cast<wchar_t>(*Narrow)`; platform `char` signedness is preserved
/// by using `c_char`, matching the original cast exactly on every supported
/// target). A null `text` appends nothing, matching the original's
/// `Narrow && *Narrow` loop guard.
///
/// # Safety
/// `dest` must be writable for `count`; `text` must be null or a
/// NUL-terminated narrow string, readable.
pub unsafe fn append_narrow_text(
    dest: *mut Tchar,
    count: usize,
    out: &mut usize,
    text: *const std::ffi::c_char,
) -> bool {
    if text.is_null() {
        return true;
    }
    let mut index = 0usize;
    loop {
        let byte = unsafe { *text.add(index) };
        if byte == 0 {
            return true;
        }
        if !unsafe { put(dest, count, out, byte as Tchar) } {
            return false;
        }
        index += 1;
    }
}

/// Pure decimal rendering of a signed int; mirrors `appVswprintfAppendInt`,
/// including `INT_MIN`'s magnitude via wrapping subtraction (matching the
/// original's `0u - static_cast<unsigned int>(Value)`).
///
/// # Safety
/// `dest` must be writable for `count`.
pub unsafe fn append_int(dest: *mut Tchar, count: usize, out: &mut usize, value: i32) -> bool {
    let mut digits = [0 as Tchar; 16];
    let mut digit_count = 0usize;
    let mut magnitude: u32 = if value < 0 {
        0u32.wrapping_sub(value as u32)
    } else {
        value as u32
    };
    loop {
        digits[digit_count] = (b'0' as u32 + magnitude % 10) as Tchar;
        digit_count += 1;
        magnitude /= 10;
        if magnitude == 0 {
            break;
        }
    }
    if value < 0 && !unsafe { put(dest, count, out, b'-' as Tchar) } {
        return false;
    }
    while digit_count > 0 {
        digit_count -= 1;
        if !unsafe { put(dest, count, out, digits[digit_count]) } {
            return false;
        }
    }
    true
}

/// Lenient Segment append: the original `PutSeg` lambda, which silently
/// truncates rather than failing the whole directive. Flags, literal
/// width/precision digits, length modifiers, and the conversion character
/// itself all use this lenient form; only `*`-resolved digits use the
/// strict, overflow-signaling [`append_int`].
fn put_seg(segment: *mut Tchar, seg_cap: usize, seg_out: &mut usize, ch: Tchar) {
    if *seg_out + 1 < seg_cap {
        unsafe {
            *segment.add(*seg_out) = ch;
        }
        *seg_out += 1;
    }
}

unsafe fn scan_flags(format: *const Tchar, idx: &mut usize, segment: *mut Tchar, seg_cap: usize, seg_out: &mut usize) {
    loop {
        let ch = unsafe { *format.add(*idx) };
        if ch != b'-' as Tchar
            && ch != b'+' as Tchar
            && ch != b' ' as Tchar
            && ch != b'#' as Tchar
            && ch != b'0' as Tchar
        {
            return;
        }
        put_seg(segment, seg_cap, seg_out, ch);
        *idx += 1;
    }
}

unsafe fn scan_digits(format: *const Tchar, idx: &mut usize, segment: *mut Tchar, seg_cap: usize, seg_out: &mut usize) {
    loop {
        let ch = unsafe { *format.add(*idx) };
        if ch < b'0' as Tchar || ch > b'9' as Tchar {
            return;
        }
        put_seg(segment, seg_cap, seg_out, ch);
        *idx += 1;
    }
}

/// Resolves a `*`-fetched width or precision into `Segment` digits. Width:
/// negative renders a leading `-` flag (lenient `PutSeg`) then the magnitude;
/// precision: negative means "no precision" and renders nothing. Both render
/// digits through the strict, overflow-signaling [`append_int`], matching the
/// original's direct `appVswprintfAppendInt(Segment, ...)` call (not
/// `PutSeg`). Width's magnitude computation mirrors the original's
/// `Width < 0 ? -Width : Width` with wrapping negation, reproducing its
/// `INT_MIN` behavior (a second `-` from `append_int`'s own sign handling)
/// exactly rather than diverging on that edge case.
fn resolve_star_arg(segment: *mut Tchar, seg_cap: usize, seg_out: &mut usize, value: i32, is_width: bool) -> bool {
    if is_width {
        if value < 0 {
            put_seg(segment, seg_cap, seg_out, b'-' as Tchar);
        }
        let resolved = if value < 0 { value.wrapping_neg() } else { value };
        unsafe { append_int(segment, seg_cap, seg_out, resolved) }
    } else if value >= 0 {
        unsafe { append_int(segment, seg_cap, seg_out, value) }
    } else {
        true
    }
}

unsafe fn scan_length_and_conversion(
    format: *const Tchar,
    idx: &mut usize,
    segment: *mut Tchar,
    seg_cap: usize,
    seg_out: &mut usize,
    length_out: &mut Length,
    conversion_out: &mut Tchar,
) -> DirectiveAction {
    let mut length = Length::None;
    loop {
        let ch = unsafe { *format.add(*idx) };
        if ch == b'l' as Tchar {
            length = if length == Length::L { Length::Ll } else { Length::L };
        } else if ch == b'h' as Tchar {
            length = if length == Length::H { Length::Hh } else { Length::H };
        } else if ch == b'z' as Tchar {
            length = Length::Z;
        } else if ch == b'j' as Tchar {
            length = Length::J;
        } else if ch == b't' as Tchar {
            length = Length::T;
        } else if ch == b'L' as Tchar {
            length = Length::BigL;
        } else {
            break;
        }
        put_seg(segment, seg_cap, seg_out, ch);
        *idx += 1;
    }
    let conversion = unsafe { *format.add(*idx) };
    if conversion == 0 {
        return DirectiveAction::Overflow;
    }
    put_seg(segment, seg_cap, seg_out, conversion);
    unsafe {
        *segment.add(*seg_out) = 0;
    }
    *idx += 1;
    *length_out = length;
    *conversion_out = conversion;
    DirectiveAction::Done
}

unsafe fn scan_precision_and_tail(
    format: *const Tchar,
    idx: &mut usize,
    segment: *mut Tchar,
    seg_cap: usize,
    seg_out: &mut usize,
    length_out: &mut Length,
    conversion_out: &mut Tchar,
) -> DirectiveAction {
    if unsafe { *format.add(*idx) } == b'.' as Tchar {
        put_seg(segment, seg_cap, seg_out, b'.' as Tchar);
        *idx += 1;
        if unsafe { *format.add(*idx) } == b'*' as Tchar {
            *idx += 1;
            return DirectiveAction::NeedPrecisionArg;
        }
        unsafe { scan_digits(format, idx, segment, seg_cap, seg_out) };
    }
    unsafe { scan_length_and_conversion(format, idx, segment, seg_cap, seg_out, length_out, conversion_out) }
}

/// Parses (or resumes parsing) one `%` directive. See the module docs for
/// the `stage`/`arg_value` resume protocol.
///
/// # Safety
/// `format` must be NUL-terminated and readable from `*in_out` onward.
/// `segment` must be writable for `seg_cap` elements; on `stage == 0` it must
/// already hold `%` at index 0 with `*seg_out == 1` (the driver pushes the
/// opening `%` itself before the first call for a directive).
/// `length_out`/`conversion_out` are written only when the result is `Done`.
#[allow(clippy::too_many_arguments)]
pub unsafe fn parse_directive(
    format: *const Tchar,
    in_out: *mut usize,
    segment: *mut Tchar,
    seg_cap: usize,
    seg_out: *mut usize,
    stage: u32,
    arg_value: i32,
    length_out: *mut Length,
    conversion_out: *mut Tchar,
) -> DirectiveAction {
    let mut idx = unsafe { *in_out };
    let mut seg = unsafe { *seg_out };
    let mut length = Length::None;
    let mut conversion: Tchar = 0;
    let action = unsafe {
        match stage {
            0 => {
                scan_flags(format, &mut idx, segment, seg_cap, &mut seg);
                if *format.add(idx) == b'*' as Tchar {
                    idx += 1;
                    DirectiveAction::NeedWidthArg
                } else {
                    scan_digits(format, &mut idx, segment, seg_cap, &mut seg);
                    scan_precision_and_tail(format, &mut idx, segment, seg_cap, &mut seg, &mut length, &mut conversion)
                }
            }
            1 => {
                if resolve_star_arg(segment, seg_cap, &mut seg, arg_value, true) {
                    scan_precision_and_tail(format, &mut idx, segment, seg_cap, &mut seg, &mut length, &mut conversion)
                } else {
                    DirectiveAction::Overflow
                }
            }
            2 => {
                if resolve_star_arg(segment, seg_cap, &mut seg, arg_value, false) {
                    scan_length_and_conversion(format, &mut idx, segment, seg_cap, &mut seg, &mut length, &mut conversion)
                } else {
                    DirectiveAction::Overflow
                }
            }
            _ => DirectiveAction::Overflow,
        }
    };
    unsafe {
        *in_out = idx;
        *seg_out = seg;
        if action == DirectiveAction::Done {
            *length_out = length;
            *conversion_out = conversion;
        }
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_wide(s: &str) -> Vec<Tchar> {
        s.chars().map(|c| c as Tchar).chain(std::iter::once(0)).collect()
    }

    fn wide_to_string(buf: &[Tchar]) -> String {
        buf.iter()
            .take_while(|&&c| c != 0)
            .map(|&c| char::from_u32(c as u32).unwrap())
            .collect()
    }

    /// Drives `parse_directive` to completion the way the C++ shell does,
    /// resolving `*` width/precision with the supplied fake `va_arg` values.
    /// `start` addresses the character right after the directive's `%`.
    fn drive(
        format: &[Tchar],
        start: usize,
        mut star_args: impl Iterator<Item = i32>,
    ) -> (usize, String, Length, Tchar, DirectiveAction) {
        let mut segment = vec![0 as Tchar; 64];
        segment[0] = b'%' as Tchar;
        let mut seg_out = 1usize;
        let mut idx = start;
        let mut stage = 0u32;
        let mut arg_value = 0i32;
        loop {
            let mut length = Length::None;
            let mut conversion: Tchar = 0;
            let action = unsafe {
                parse_directive(
                    format.as_ptr(),
                    &mut idx,
                    segment.as_mut_ptr(),
                    segment.len(),
                    &mut seg_out,
                    stage,
                    arg_value,
                    &mut length,
                    &mut conversion,
                )
            };
            match action {
                DirectiveAction::NeedWidthArg => {
                    arg_value = star_args.next().expect("width arg expected");
                    stage = 1;
                }
                DirectiveAction::NeedPrecisionArg => {
                    arg_value = star_args.next().expect("precision arg expected");
                    stage = 2;
                }
                DirectiveAction::Done | DirectiveAction::Overflow => {
                    return (idx, wide_to_string(&segment), length, conversion, action);
                }
            }
        }
    }

    #[test]
    fn literal_run_appends_via_put() {
        let dest_buf = to_wide("hello");
        let mut dest = vec![0 as Tchar; 8];
        let mut out = 0usize;
        assert!(unsafe { append_text_n(dest.as_mut_ptr(), dest.len(), &mut out, dest_buf.as_ptr(), 5) });
        assert_eq!(out, 5);
        assert_eq!(wide_to_string(&dest), "hello");
    }

    #[test]
    fn plain_d_conversion() {
        let format = to_wide("%d");
        let (idx, segment, length, conversion, action) = drive(&format, 1, std::iter::empty());
        assert_eq!(action, DirectiveAction::Done);
        assert_eq!(segment, "%d");
        assert_eq!(length, Length::None);
        assert_eq!(conversion, b'd' as Tchar);
        assert_eq!(idx, format.len() - 1); // one past the 'd', before the NUL
    }

    #[test]
    fn width_and_precision_literal_float() {
        let format = to_wide("%5.2f");
        let (_idx, segment, _length, conversion, action) = drive(&format, 1, std::iter::empty());
        assert_eq!(action, DirectiveAction::Done);
        assert_eq!(segment, "%5.2f");
        assert_eq!(conversion, b'f' as Tchar);
    }

    #[test]
    fn width_star_positive() {
        let format = to_wide("%*d");
        let (_idx, segment, _length, conversion, action) = drive(&format, 1, [7].into_iter());
        assert_eq!(action, DirectiveAction::Done);
        assert_eq!(segment, "%7d");
        assert_eq!(conversion, b'd' as Tchar);
    }

    #[test]
    fn width_star_negative_renders_left_flag() {
        let format = to_wide("%*d");
        let (_idx, segment, _length, _conversion, action) = drive(&format, 1, [-12].into_iter());
        assert_eq!(action, DirectiveAction::Done);
        assert_eq!(segment, "%-12d");
    }

    #[test]
    fn precision_star() {
        let format = to_wide("%.*f");
        let (_idx, segment, _length, conversion, action) = drive(&format, 1, [3].into_iter());
        assert_eq!(action, DirectiveAction::Done);
        assert_eq!(segment, "%.3f");
        assert_eq!(conversion, b'f' as Tchar);
    }

    #[test]
    fn precision_star_negative_omits_precision() {
        let format = to_wide("%.*f");
        let (_idx, segment, _length, _conversion, action) = drive(&format, 1, [-1].into_iter());
        assert_eq!(action, DirectiveAction::Done);
        assert_eq!(segment, "%.f");
    }

    #[test]
    fn width_and_precision_both_star() {
        let format = to_wide("%*.*f");
        let (_idx, segment, _length, conversion, action) = drive(&format, 1, [8, 4].into_iter());
        assert_eq!(action, DirectiveAction::Done);
        assert_eq!(segment, "%8.4f");
        assert_eq!(conversion, b'f' as Tchar);
    }

    #[test]
    fn length_modifiers() {
        for (text, expected) in [("%ld", Length::L), ("%lld", Length::Ll), ("%hhd", Length::Hh), ("%hd", Length::H)] {
            let format = to_wide(text);
            let (_idx, segment, length, conversion, action) = drive(&format, 1, std::iter::empty());
            assert_eq!(action, DirectiveAction::Done, "{text}");
            assert_eq!(segment, text, "{text}");
            assert_eq!(length, expected, "{text}");
            assert_eq!(conversion, b'd' as Tchar, "{text}");
        }
    }

    #[test]
    fn string_and_char_and_narrow_conversions() {
        for text in ["%s", "%hs", "%c"] {
            let format = to_wide(text);
            let (_idx, segment, _length, _conversion, action) = drive(&format, 1, std::iter::empty());
            assert_eq!(action, DirectiveAction::Done, "{text}");
            assert_eq!(segment, text, "{text}");
        }
    }

    #[test]
    fn unknown_conversion_still_parses_and_passes_through() {
        let format = to_wide("%q");
        let (_idx, segment, _length, conversion, action) = drive(&format, 1, std::iter::empty());
        assert_eq!(action, DirectiveAction::Done);
        assert_eq!(segment, "%q");
        assert_eq!(conversion, b'q' as Tchar);
    }

    #[test]
    fn missing_conversion_overflows() {
        let format = to_wide("%5");
        let (_idx, _segment, _length, _conversion, action) = drive(&format, 1, std::iter::empty());
        assert_eq!(action, DirectiveAction::Overflow);
    }

    #[test]
    fn star_width_overflows_tiny_segment() {
        let format = to_wide("%*d");
        let mut segment = vec![0 as Tchar; 3]; // room for '%' + 1 digit + NUL only
        segment[0] = b'%' as Tchar;
        let mut seg_out = 1usize;
        let mut idx = 1usize;
        let mut length = Length::None;
        let mut conversion: Tchar = 0;
        let action = unsafe {
            parse_directive(
                format.as_ptr(),
                &mut idx,
                segment.as_mut_ptr(),
                segment.len(),
                &mut seg_out,
                0,
                0,
                &mut length,
                &mut conversion,
            )
        };
        assert_eq!(action, DirectiveAction::NeedWidthArg);
        let action = unsafe {
            parse_directive(
                format.as_ptr(),
                &mut idx,
                segment.as_mut_ptr(),
                segment.len(),
                &mut seg_out,
                1,
                12345,
                &mut length,
                &mut conversion,
            )
        };
        assert_eq!(action, DirectiveAction::Overflow);
    }

    #[test]
    fn put_overflow_at_capacity_boundary() {
        let mut dest = vec![0 as Tchar; 2];
        let mut out = 0usize;
        assert!(unsafe { put(dest.as_mut_ptr(), dest.len(), &mut out, b'a' as Tchar) });
        assert!(!unsafe { put(dest.as_mut_ptr(), dest.len(), &mut out, b'b' as Tchar) });
    }

    #[test]
    fn append_text_overflow_leaves_partial_writes() {
        let text = to_wide("abcdef");
        let mut dest = vec![0 as Tchar; 4];
        let mut out = 0usize;
        assert!(!unsafe { append_text(dest.as_mut_ptr(), dest.len(), &mut out, text.as_ptr()) });
    }

    #[test]
    fn append_int_positive_and_negative() {
        let mut dest = vec![0 as Tchar; 16];
        let mut out = 0usize;
        assert!(unsafe { append_int(dest.as_mut_ptr(), dest.len(), &mut out, 1234) });
        assert_eq!(wide_to_string(&dest), "1234");

        let mut dest = vec![0 as Tchar; 16];
        let mut out = 0usize;
        assert!(unsafe { append_int(dest.as_mut_ptr(), dest.len(), &mut out, -42) });
        assert_eq!(wide_to_string(&dest), "-42");
    }

    #[test]
    fn append_int_min_matches_original_wrapping_behavior() {
        // Mirrors `appVswprintfAppendInt`'s own internal sign handling when
        // called directly with a still-negative value, as happens for the
        // original `%*d` call site's `-Width` on `Width == INT_MIN` under
        // typical (non-UBSan) two's-complement wraparound.
        let mut dest = vec![0 as Tchar; 16];
        let mut out = 0usize;
        assert!(unsafe { append_int(dest.as_mut_ptr(), dest.len(), &mut out, i32::MIN) });
        assert_eq!(wide_to_string(&dest), "-2147483648");
    }

    #[test]
    fn append_narrow_text_widens_bytes_and_handles_null() {
        let mut dest = vec![0 as Tchar; 8];
        let mut out = 0usize;
        let narrow = std::ffi::CString::new("hi").unwrap();
        assert!(unsafe { append_narrow_text(dest.as_mut_ptr(), dest.len(), &mut out, narrow.as_ptr()) });
        assert_eq!(wide_to_string(&dest), "hi");

        let mut dest = vec![0 as Tchar; 8];
        let mut out = 0usize;
        assert!(unsafe { append_narrow_text(dest.as_mut_ptr(), dest.len(), &mut out, std::ptr::null()) });
        assert_eq!(out, 0);
    }
}
