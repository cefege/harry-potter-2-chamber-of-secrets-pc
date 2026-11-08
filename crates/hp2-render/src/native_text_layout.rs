//! Provider-neutral native typography policy. SDK providers supply shaping facts;
//! source mapping, role selection and pixel-boundary rules remain here.
use core::ffi::c_void;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Hp2TextLayoutRange {
    pub location: isize,
    pub length: isize,
}
pub type Range = Hp2TextLayoutRange;

pub struct Prepared {
    pub text: Vec<u16>,
    pub source_ends: Vec<i32>,
    pub underlines: Vec<Range>,
}

/// Read native unsigned TCHAR units, not UTF-16 pairs: this preserves the
/// original scalar-per-TCHAR conversion even on a 16-bit TCHAR target.
/// # Safety
/// `text` addresses `length` readable units of the indicated width.
pub unsafe fn units(text: *const c_void, length: usize, unit_bytes: u32) -> Vec<u32> {
    (0..length)
        .map(|i| unsafe { unit(text, i, unit_bytes) })
        .collect()
}

unsafe fn unit(text: *const c_void, index: usize, width: u32) -> u32 {
    unsafe {
        match width {
            1 => *text.cast::<u8>().add(index) as u32,
            2 => *text.cast::<u16>().add(index) as u32,
            4 => *text.cast::<u32>().add(index),
            _ => 0,
        }
    }
}

/// # Safety
/// A non-null `text` addresses `length` readable native units.
pub unsafe fn preprocess(
    text: *const c_void,
    length: i32,
    unit_bytes: u32,
    amp: bool,
) -> Option<Prepared> {
    if text.is_null() || length < 0 || !matches!(unit_bytes, 1 | 2 | 4) {
        return None;
    }
    let mut out = Prepared {
        text: Vec::with_capacity(length as usize),
        source_ends: Vec::with_capacity(length as usize),
        underlines: Vec::new(),
    };
    let mut index = 0usize;
    while index < length as usize {
        let scalar = unsafe { unit(text, index, unit_bytes) };
        if scalar == 0 {
            break;
        }
        let escaped = amp && scalar == b'&' as u32;
        let scalar = if escaped {
            index += 1;
            if index == length as usize {
                break;
            }
            let next = unsafe { unit(text, index, unit_bytes) };
            if next == 0 {
                break;
            }
            next
        } else {
            scalar
        };
        let start = out.text.len();
        let ch = char::from_u32(scalar).unwrap_or('\u{fffd}');
        let mut encoded = [0u16; 2];
        let encoded = ch.encode_utf16(&mut encoded);
        out.text.extend_from_slice(encoded);
        out.source_ends
            .extend(core::iter::repeat_n(index as i32 + 1, encoded.len()));
        if escaped && scalar != b'&' as u32 {
            out.underlines.push(Range {
                location: start as isize,
                length: encoded.len() as isize,
            });
        }
        index += 1;
    }
    Some(out)
}

const PACKAGE_ROLES: &[(&[u8], u32)] = &[
    (b"tinyinkfont", 0),
    (b"smallinkfont", 1),
    (b"medinkfont", 2),
    (b"biginkfont", 4),
    (b"hugeinkfont", 5),
    (b"tahoma10", 1),
    (b"tahomab10", 3),
    (b"tahoma20", 4),
    (b"tahomab20", 5),
    (b"tahoma30", 4),
    (b"tahomab30", 5),
    (b"fonthpmenumedium", 6),
    (b"fonthpmenularge", 7),
];
fn package_role(name: &[u32]) -> Option<u32> {
    PACKAGE_ROLES.iter().find_map(|&(expected, role)| {
        (name.len() == expected.len()
            && name.iter().zip(expected).all(|(&a, &b)| {
                a == b as u32 || (a >= b'A' as u32 && a <= b'Z' as u32 && a + 32 == b as u32)
            }))
        .then_some(role)
    })
}
pub fn role(name: &[u32], explicit_family: bool) -> u32 {
    package_role(name).unwrap_or(if explicit_family { 8 } else { 2 })
}
pub fn known_package(name: &[u32]) -> bool {
    package_role(name).is_some()
}
pub fn role_size(role: u32) -> i32 {
    match role {
        0 => 10,
        1 => 12,
        4 | 6 | 9 => 18,
        5 | 7 => 24,
        _ => 14,
    }
}
/// Fallback point size for a legacy (family-less) bitmap `UFont`: the max
/// `VSize` across all its pages, matching both providers' original C++
/// `Page()` scan. `page_fn` returns the max `VSize` of one page's
/// characters (0 if the page is empty or missing); `font` is the opaque
/// `UFont*` passed straight through. Falls back to `role_size(2)` (the Body
/// role) when every page is empty, same as both providers always did.
/// # Safety
/// `page_fn` must be callable with `font` and any `page` in `0..page_count`.
pub unsafe fn legacy_bitmap_size(
    font: *const c_void,
    page_count: i32,
    page_fn: unsafe extern "C" fn(*const c_void, i32) -> i32,
) -> i32 {
    let mut size = 0;
    for page in 0..page_count {
        size = size.max(unsafe { page_fn(font, page) });
    }
    if size <= 0 { role_size(2) } else { size }
}
pub fn bold(role: u32) -> bool {
    matches!(role, 3 | 5)
}
pub fn safe_float(v: f32) -> bool {
    v.is_finite() && (v as f64) >= i32::MIN as f64 && (v as f64) <= i32::MAX as f64
}
pub fn valid_point_size(v: f64) -> bool {
    v.is_finite() && v > 0.0 && v <= u32::MAX as f64 / 64.0
}
pub fn point_size64(v: f64) -> Option<u32> {
    let scaled = v * 64.0;
    (scaled.is_finite() && scaled > 0.0 && scaled <= u32::MAX as f64 - 0.5)
        .then(|| scaled.round() as u32)
}
pub fn visible_end(
    clusters: impl Iterator<Item = (Range, i32)>,
    visible: i32,
    full: usize,
) -> usize {
    if visible <= 0 {
        return full;
    }
    let mut allowed = 0isize;
    for (range, source_end) in clusters {
        if range.location > allowed || source_end > visible {
            break;
        }
        allowed = allowed.max(range.location + range.length);
    }
    allowed as usize
}

pub fn raster_rectangle(bounds: [f64; 4], drawable: bool, page: i32) -> Option<[f64; 4]> {
    if page <= 2 || !drawable || !bounds.iter().all(|v| v.is_finite()) {
        return None;
    }
    let [min_x, min_y, max_x, max_y] = bounds;
    let min_x = min_x.floor();
    let min_y = min_y.floor();
    let width = max_x.ceil() - min_x;
    let height = max_y.ceil() - min_y;
    if !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
        || width > (page - 2) as f64
        || height > (page - 2) as f64
    {
        return None;
    }
    Some([min_x, min_y, width, height])
}
pub fn draw_offset(position: [f64; 2], bounds: [f64; 4]) -> [f64; 2] {
    [
        position[0] + bounds[0].floor(),
        position[1] - bounds[3].ceil(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaped_supplementary_scalar_keeps_original_source_end() {
        let source = [
            b'&' as u32,
            0x1f9d9,
            b'&' as u32,
            b'&' as u32,
            b'&' as u32,
            0,
        ];
        let p = unsafe { preprocess(source.as_ptr().cast(), 6, 4, true) }.unwrap();
        assert_eq!(p.text, [0xd83e, 0xddd9, b'&' as u16]);
        assert_eq!(p.source_ends, [2, 2, 4]);
        assert_eq!(
            p.underlines,
            [Range {
                location: 0,
                length: 2
            }]
        );
    }

    #[test]
    fn native_units_are_scalars_not_surrogate_pairs() {
        let source = [0xd83eu16, 0xddd9, 0, b'x' as u16];
        let p = unsafe { preprocess(source.as_ptr().cast(), 4, 2, false) }.unwrap();
        assert_eq!(p.text, [0xfffd, 0xfffd]);
        assert_eq!(p.source_ends, [1, 2]);
    }

    #[test]
    fn visible_source_never_splits_a_ligature_or_surrogate_cluster() {
        let clusters = [
            (
                Range {
                    location: 0,
                    length: 2,
                },
                2,
            ),
            (
                Range {
                    location: 2,
                    length: 3,
                },
                5,
            ),
        ];
        assert_eq!(visible_end(clusters.into_iter(), 1, 5), 0);
        assert_eq!(visible_end(clusters.into_iter(), 4, 5), 2);
        assert_eq!(visible_end(clusters.into_iter(), 0, 5), 5);
    }

    #[test]
    fn raster_rounding_preserves_negative_bearings_and_padding_limit() {
        let bounds = [-0.2, -3.1, 4.2, 8.1];
        assert_eq!(
            raster_rectangle(bounds, true, 15),
            Some([-1.0, -4.0, 6.0, 13.0])
        );
        assert_eq!(raster_rectangle(bounds, true, 14), None);
        assert_eq!(draw_offset([7.5, 12.0], bounds), [6.5, 3.0]);
        assert_eq!(
            raster_rectangle([0.0, 0.0, f64::INFINITY, 1.0], true, 1024),
            None
        );
    }

    #[test]
    fn public_role_names_and_explicit_families_stay_distinct() {
        let name: Vec<u32> = "TaHoMaB20".chars().map(u32::from).collect();
        assert_eq!(role(&name, true), 5);
        assert_eq!(role_size(role(&name, true)), 24);
        assert!(bold(role(&name, true)));
        assert_eq!(role(&[], true), 8);
        assert_eq!(role(&[], false), 2);
    }
}
