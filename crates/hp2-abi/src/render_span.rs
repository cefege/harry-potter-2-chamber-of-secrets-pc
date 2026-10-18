//! `#[repr(C)]` mirrors of the span-buffer types in
//! `Render/Inc/UnSpan.h` (`FSpan`, `FRasterSpan`, `FSpanBuffer`) and of the
//! `FMemMark` they embed (`Core/Inc/UnMem.h`). Used by the span port
//! (`hp2-render::span`). Layout was probed with clang++ against the pinned
//! headers (`sizeof`/`offsetof`), and the same facts are pinned on the C++
//! side by `static_assert`s in `UnSpan.h`.

use std::ffi::c_void;

/// `FMemMark { FMemStack* Mem; BYTE* Top; FTaggedMemory* SavedChunk; }`.
/// 24 bytes. The pointees are C++ objects; Rust only forwards them.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FMemMark {
    pub mem: *mut c_void,
    pub top: *mut u8,
    pub saved_chunk: *mut c_void,
}

const _: () = assert!(size_of::<FMemMark>() == 24);
const _: () = assert!(std::mem::offset_of!(FMemMark, mem) == 0);
const _: () = assert!(std::mem::offset_of!(FMemMark, top) == 8);
const _: () = assert!(std::mem::offset_of!(FMemMark, saved_chunk) == 16);

/// `FSpan { INT Start, End; FSpan* Next; }`. 16 bytes, 8-byte aligned.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FSpan {
    pub start: i32,
    pub end: i32,
    pub next: *mut FSpan,
}

const _: () = assert!(size_of::<FSpan>() == 16);
const _: () = assert!(std::mem::offset_of!(FSpan, start) == 0);
const _: () = assert!(std::mem::offset_of!(FSpan, end) == 4);
const _: () = assert!(std::mem::offset_of!(FSpan, next) == 8);

/// `FRasterSpan { INT X[2]; }`. 8 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FRasterSpan {
    pub x: [i32; 2],
}

const _: () = assert!(size_of::<FRasterSpan>() == 8);
const _: () = assert!(std::mem::offset_of!(FRasterSpan, x) == 0);

/// `FSpanBuffer` (`Render/Inc/UnSpan.h`). 56 bytes.
///
/// `index` is `FSpan** Index` (one head pointer per scanline, relative to
/// `StartY`). `mem` is `FMemStack* Mem`. `mark` is the `FMemMark` taken by
/// `AllocIndex`, popped by `Release`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FSpanBuffer {
    pub start_y: i32,
    pub end_y: i32,
    pub valid_lines: i32,
    pub index: *mut *mut FSpan,
    pub mem: *mut c_void,
    pub mark: FMemMark,
}

const _: () = assert!(size_of::<FSpanBuffer>() == 56);
const _: () = assert!(std::mem::offset_of!(FSpanBuffer, start_y) == 0);
const _: () = assert!(std::mem::offset_of!(FSpanBuffer, end_y) == 4);
const _: () = assert!(std::mem::offset_of!(FSpanBuffer, valid_lines) == 8);
const _: () = assert!(std::mem::offset_of!(FSpanBuffer, index) == 16);
const _: () = assert!(std::mem::offset_of!(FSpanBuffer, mem) == 24);
const _: () = assert!(std::mem::offset_of!(FSpanBuffer, mark) == 32);
