//! `FRasterPoly` header (`Render/Inc/UnSpan.h`): `INT StartY; INT EndY;
//! FRasterSpan Lines[ZEROARRAY];`. The variable-length `Lines` array follows
//! the header at offset 8; each allocation holds `EndY - StartY` spans there.
//! `FRasterSpan` itself lives in `render_span`.

/// Header of `FRasterPoly`. The span array is not part of this struct: it
/// starts at byte offset 8 and holds `end_y - start_y` `FRasterSpan` entries.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FRasterPolyHeader {
    pub start_y: i32,
    pub end_y: i32,
}

const _: () = assert!(size_of::<FRasterPolyHeader>() == 8);
const _: () = assert!(align_of::<FRasterPolyHeader>() == 4);
const _: () = assert!(std::mem::offset_of!(FRasterPolyHeader, start_y) == 0);
const _: () = assert!(std::mem::offset_of!(FRasterPolyHeader, end_y) == 4);
