//! Port of `HarryPotter2/Unreal/Render/Src/UnSpan.cpp` (span buffers).
//!
//! Line-by-line transcription over the `#[repr(C)]` mirrors in
//! `hp2_abi::render_span`. The lists are engine-owned raw-pointer graphs; each
//! `pub unsafe fn` documents the invariants it relies on, and the whole walk is
//! written with raw pointers so the C++ pointer-to-link idioms (`FSpan**
//! PrevLink`) survive unchanged.
//!
//! Memory comes from the engine's `FMemStack` through [`SpanAlloc`].
//! Production binds it to the services table; tests bind it to an arena.
#![allow(unsafe_op_in_unsafe_fn)]

use std::ffi::c_void;
use std::ptr;

use hp2_abi::engine::generated::FMemStack;
use hp2_abi::render_span::{FMemMark, FRasterSpan, FSpan, FSpanBuffer};
use hp2_core::mem::{self, Hp2MemServices};

/// Native allocator and warning-device leaves used by span buffers.
#[repr(C)]
pub struct Hp2SpanServices {
    pub memory: *const Hp2MemServices,
    pub slow_checks: i32,
    pub log_warning: unsafe extern "C" fn(*const u8, usize),
}

/// Formatting/error-device leaf. Zero means the error device returned normally.
pub type Hp2SpanDiagnostic =
    unsafe extern "C" fn(*const c_void, u32, i32, i32, i32, i32, i32) -> i32;

unsafe fn clone_allocate(
    services: &Hp2SpanServices,
    stack: *mut c_void,
    bytes: usize,
    alignment: i32,
) -> Result<*mut u8, i32> {
    let size = mem::checked_int_size(&*services.memory, bytes, 3)?;
    mem::push_bytes(
        &mut *stack.cast::<FMemStack>(),
        size,
        alignment,
        services.slow_checks != 0,
        c"UnMem.h",
        &*services.memory,
    )
}

/// Deep placement-copy, including index allocation before mark capture.
///
/// # Safety
/// `this` is writable construction storage; `source` and `stack` obey the
/// native constructor's pointer/size contract. Callbacks may mutate native
/// state; no span-buffer or node reference is retained across them.
pub unsafe fn clone_buffer(
    services: &Hp2SpanServices,
    this: *mut FSpanBuffer,
    source: *const FSpanBuffer,
    stack: *mut c_void,
) -> Result<(), i32> {
    ptr::addr_of_mut!((*this).start_y).write((*source).start_y);
    ptr::addr_of_mut!((*this).end_y).write((*source).end_y);
    ptr::addr_of_mut!((*this).valid_lines).write((*source).valid_lines);
    let count = (*this).end_y.wrapping_sub((*this).start_y);
    let index = clone_allocate(
        services,
        stack,
        (count as usize).wrapping_mul(size_of::<*mut FSpan>()),
        DEFAULT_ALIGNMENT as i32,
    )?;
    ptr::addr_of_mut!((*this).index).write(index.cast());
    ptr::addr_of_mut!((*this).mem).write(stack);
    mem::mark_init(ptr::addr_of_mut!((*this).mark), stack.cast());
    let mut i = 0i32;
    while i < (*this).end_y.wrapping_sub((*this).start_y) {
        let mut previous = (*this).index.offset(i as isize);
        let mut other = *(*source).index.offset(i as isize);
        while !other.is_null() {
            let node =
                clone_allocate(services, (*this).mem, size_of::<FSpan>(), 4)?.cast::<FSpan>();
            ptr::addr_of_mut!((*node).start).write((*other).start);
            ptr::addr_of_mut!((*node).end).write((*other).end);
            // Native node allocations request alignment 4, including fresh chunks.
            previous.write_unaligned(node);
            previous = ptr::addr_of_mut!((*node).next);
            other = (*other).next;
        }
        previous.write_unaligned(ptr::null_mut());
        i = i.wrapping_add(1);
    }
    Ok(())
}

/// Original four diagnostics: 0 empty, 1 nonempty, 2 valid, 3 good enough.
///
/// # Safety
/// All visited pointers must remain valid, including after a returning error
/// callback. Index is captured once; bounds, geometry, and next links are
/// deliberately read at the original live expression points.
pub unsafe fn diagnose(
    this: *mut FSpanBuffer,
    name: *const c_void,
    mode: u32,
    report: Hp2SpanDiagnostic,
) -> Result<(), i32> {
    let mut index = (*this).index;
    let mut i = (*this).start_y;
    let mut count = 0i32;
    while i < (*this).end_y {
        let mut previous_end = -1000;
        let mut node = *index;
        index = index.add(1);
        while !node.is_null() {
            if mode == 0 {
                let status = report(
                    name,
                    0,
                    (*this).start_y,
                    i,
                    (*this).end_y,
                    (*node).start,
                    (*node).end,
                );
                if status != 0 {
                    return Err(status);
                }
            } else if mode == 1 {
                if (*node).start >= (*node).end {
                    let status =
                        report(name, 1, (*node).end.wrapping_sub((*node).start), 0, 0, 0, 0);
                    if status != 0 {
                        return Err(status);
                    }
                }
                count = 1;
            } else {
                if i == (*this).start_y || i == (*this).end_y.wrapping_sub(1) {
                    if previous_end != -1000 && previous_end >= (*node).start {
                        let status = report(
                            name,
                            3,
                            previous_end.wrapping_sub((*node).start),
                            i.wrapping_sub((*this).start_y),
                            (*this).end_y.wrapping_sub((*this).start_y),
                            0,
                            0,
                        );
                        if status != 0 {
                            return Err(status);
                        }
                    }
                    if (*node).start >= (*node).end {
                        let status = report(
                            name,
                            4,
                            (*node).end.wrapping_sub((*node).start),
                            i.wrapping_sub((*this).start_y),
                            (*this).end_y.wrapping_sub((*this).start_y),
                            0,
                            0,
                        );
                        if status != 0 {
                            return Err(status);
                        }
                    }
                    previous_end = (*node).end;
                }
            }
            node = (*node).next;
            if mode >= 2 {
                count = count.wrapping_add(1);
            }
        }
        i = i.wrapping_add(1);
    }
    let status = if mode == 1 && count == 0 {
        report(name, 2, 0, 0, 0, 0, 0)
    } else if (mode == 2 && count != (*this).valid_lines)
        || (mode == 3 && (count == 0) != ((*this).valid_lines == 0))
    {
        report(name, 5, (*this).valid_lines, count, 0, 0, 0)
    } else {
        0
    };
    if status == 0 { Ok(()) } else { Err(status) }
}

/// `DEFAULT_ALIGNMENT` (`Core/Inc/UnMem.h`).
pub const DEFAULT_ALIGNMENT: usize = 16;

/// Engine memory operations used by the span code.
pub trait SpanAlloc {
    /// `FMemStack::PushBytes(size, align)` on the stack behind `mem`.
    ///
    /// # Safety
    /// `mem` must be a live `FMemStack*` (or the test equivalent).
    unsafe fn push(&self, mem: *mut c_void, size: usize, align: usize) -> Result<*mut u8, i32>;
    /// `Mark = FMemMark(*mem)`: placement-initialises `mark`.
    ///
    /// # Safety
    /// `mem` live, `mark` valid for writes.
    unsafe fn mark_save(&self, mem: *mut c_void, mark: *mut FMemMark);
    /// `Mark.Pop()`.
    ///
    /// # Safety
    /// `mark` must have been saved by `mark_save`.
    unsafe fn mark_pop(&self, mark: *mut FMemMark);
    /// `debugf(NAME_Warning, ...)`.
    fn warn(&self, text: &str);
}

fn count_of(n: i32) -> usize {
    usize::try_from(n).expect("negative span count")
}

/// `New<T>(*mem, count, align)`: uninitialised storage for `count` `T`s.
unsafe fn new_array<A: SpanAlloc, T>(
    a: &A,
    mem: *mut c_void,
    count: usize,
    align: usize,
) -> Result<*mut T, i32> {
    Ok(a.push(mem, count.wrapping_mul(size_of::<T>()), align)? as *mut T)
}

/// `New<FSpan>(*mem, 1, 4)`.
unsafe fn new_span<A: SpanAlloc>(a: &A, mem: *mut c_void) -> Result<*mut FSpan, i32> {
    new_array::<A, FSpan>(a, mem, 1, 4)
}

/// `UPDATE_PREVLINK_ALLOC(START,END)` on `dst`: allocate a span from
/// `dst.mem`, link it at `*prev_link`, and advance `prev_link` to its `Next`.
unsafe fn link_alloc<A: SpanAlloc>(
    a: &A,
    dst: &mut FSpanBuffer,
    prev_link: &mut *mut *mut FSpan,
    start: i32,
    end: i32,
) -> Result<(), i32> {
    let new = new_span(a, dst.mem)?;
    **prev_link = new;
    (*new).start = start;
    (*new).end = end;
    *prev_link = ptr::addr_of_mut!((*new).next);
    dst.valid_lines += 1;
    Ok(())
}

/// `FSpanBuffer::AllocIndex`: allocate a linear span buffer with no spans.
/// The index is `New<FSpan*>` and `Mark` is saved after it.
///
/// # Safety
/// `mem` must be a live `FMemStack` and every span/index pointer reachable
/// from the buffers must be valid, as for the C++ caller.
pub unsafe fn alloc_index<A: SpanAlloc>(
    a: &A,
    b: &mut FSpanBuffer,
    alloc_start_y: i32,
    alloc_end_y: i32,
    mem: *mut c_void,
) -> Result<(), i32> {
    b.mem = mem;
    b.start_y = alloc_start_y;
    b.end_y = alloc_end_y;
    b.valid_lines = 0;
    b.index = if alloc_start_y <= alloc_end_y {
        new_array::<A, *mut FSpan>(
            a,
            mem,
            count_of(alloc_end_y - alloc_start_y),
            DEFAULT_ALIGNMENT,
        )?
    } else {
        ptr::null_mut()
    };
    a.mark_save(mem, ptr::addr_of_mut!(b.mark));
    Ok(())
}

/// `FSpanBuffer::AllocIndexForScreen`: an `SYR`-line buffer where each line is
/// one span `[0, SXR)`. `Mark` is NOT set (matches the C++).
///
/// # Safety
/// `mem` must be a live `FMemStack` and every span/index pointer reachable
/// from the buffers must be valid, as for the C++ caller.
pub unsafe fn alloc_index_for_screen<A: SpanAlloc>(
    a: &A,
    b: &mut FSpanBuffer,
    sxr: i32,
    syr: i32,
    mem: *mut c_void,
) -> Result<(), i32> {
    b.mem = mem;
    b.start_y = 0;
    b.end_y = syr;
    b.valid_lines = syr;

    let n = count_of(syr);
    b.index = new_array::<A, *mut FSpan>(a, mem, n, DEFAULT_ALIGNMENT)?;
    let list = new_array::<A, FSpan>(a, mem, n, DEFAULT_ALIGNMENT)?;
    for i in 0..n {
        *b.index.add(i) = list.add(i);
        let span = list.add(i);
        (*span).start = 0;
        (*span).end = sxr;
        (*span).next = ptr::null_mut();
    }
    Ok(())
}

/// `FSpanBuffer::Release`: pop the mark taken by `alloc_index`.
///
/// # Safety
/// `mem` must be a live `FMemStack` and every span/index pointer reachable
/// from the buffers must be valid, as for the C++ caller.
pub unsafe fn release<A: SpanAlloc>(a: &A, b: &mut FSpanBuffer) {
    a.mark_pop(ptr::addr_of_mut!(b.mark));
}

/// `FSpanBuffer::GetValidRange`. Returns `(ValidStartY, ValidEndY)`.
/// `(0, 0)` when `ValidLines == 0`. The C++ walks the index unbounded; here
/// both walks stop at the buffer edge, and an all-null index (inconsistent
/// buffer) also yields `(0, 0)`.
///
/// # Safety
/// `mem` must be a live `FMemStack` and every span/index pointer reachable
/// from the buffers must be valid, as for the C++ caller.
pub unsafe fn get_valid_range(b: &FSpanBuffer) -> (i16, i16) {
    if b.valid_lines == 0 {
        return (0, 0);
    }
    let n = count_of(b.end_y - b.start_y);

    let mut new_start = b.start_y;
    let mut i = 0usize;
    while i < n && (*b.index.add(i)).is_null() {
        i += 1;
        new_start += 1;
    }

    let mut new_end = b.end_y;
    let mut j = n;
    while j > 0 && (*b.index.add(j - 1)).is_null() {
        j -= 1;
        new_end -= 1;
    }

    if i == n {
        return (0, 0);
    }
    (new_start as i16, new_end as i16)
}

/// `FSpanBuffer::BoxIsVisible`: 1 if any part of the box is visible.
///
/// Lines are visited last line first, then `Y1..Y2-1` (the C++ order). When
/// the clamped `Count <= 0` the C++ reads `Index[-1]` (UB) and returns 0; this
/// port returns 0 without the read.
///
/// # Safety
/// `mem` must be a live `FMemStack` and every span/index pointer reachable
/// from the buffers must be valid, as for the C++ caller.
pub unsafe fn box_is_visible(b: &FSpanBuffer, x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
    if y1 >= b.end_y {
        return 0;
    }
    if y2 <= b.start_y {
        return 0;
    }
    let y1 = y1.max(b.start_y);
    let y2 = y2.min(b.end_y);

    let first = count_of(y1 - b.start_y);
    let count = y2 - y1;
    if count <= 0 {
        return 0;
    }
    let count = count as usize;

    for k in 0..count {
        let mut span = if k == 0 {
            *b.index.add(first + count - 1)
        } else {
            *b.index.add(first + k - 1)
        };
        while !span.is_null() && x2 > (*span).start {
            if x1 < (*span).end {
                return 1;
            }
            span = (*span).next;
        }
    }
    0
}

/// `FSpanBuffer::CopyFromRasterUpdate`: grind a raster poly through `screen`,
/// subtracting it from the screen's spans and building the poly's own spans
/// in `this` (allocated from `this.mem`). Screen splits allocate from
/// `screen.mem`. Returns 1 if the poly is all or partly visible.
///
/// # Safety
/// `mem` must be a live `FMemStack` and every span/index pointer reachable
/// from the buffers must be valid, as for the C++ caller.
pub unsafe fn copy_from_raster_update<A: SpanAlloc>(
    a: &A,
    this: &mut FSpanBuffer,
    screen: &mut FSpanBuffer,
    raster_start_y: i32,
    raster_end_y: i32,
    raster: *const FRasterSpan,
) -> Result<i32, i32> {
    if this.start_y > raster_start_y || this.end_y < raster_end_y {
        a.warn(&format!(
            "Illegal span range <{},{}> <{},{}>",
            this.start_y, this.end_y, raster_start_y, raster_end_y
        ));
        return Ok(0);
    }

    let our_start = raster_start_y.max(screen.start_y);
    let our_end = raster_end_y.min(screen.end_y);
    let mut temp = this.index;

    // Screen and raster do not overlap: all-null output.
    if our_start >= our_end {
        for _ in this.start_y..this.end_y {
            *temp = ptr::null_mut();
            temp = temp.add(1);
        }
        return Ok(0);
    }

    for _ in this.start_y..our_start {
        *temp = ptr::null_mut();
        temp = temp.add(1);
    }

    let mut line = raster.add(count_of(our_start - raster_start_y));
    let mut screen_index = screen.index.add(count_of(our_start - screen.start_y));

    let mut accept = 0;
    for _ in our_start..our_end {
        let mut prev_screen_link = screen_index;
        let mut screen_span = *screen_index;
        screen_index = screen_index.add(1);
        let mut prev_link = temp;
        temp = temp.add(1);

        let x0 = (*line).x[0];
        let x1 = (*line).x[1];

        'next_line: {
            // Skip if this screen span is already full, or if the raster is empty.
            if screen_span.is_null() || x1 <= x0 {
                break 'next_line;
            }

            // Skip past all spans that occur before the raster.
            while (*screen_span).end <= x0 {
                prev_screen_link = ptr::addr_of_mut!((*screen_span).next);
                screen_span = (*screen_span).next;
                if screen_span.is_null() {
                    break 'next_line;
                }
            }

            // This span straddles the raster's starting point.
            if (*screen_span).start < x0 {
                accept = 1;
                link_alloc(a, this, &mut prev_link, x0, x1.min((*screen_span).end))?;

                // Span entirely encloses the raster: split it in two.
                if (*screen_span).end > x1 {
                    let new_screen_span = new_span(a, screen.mem)?;
                    (*new_screen_span).start = x1;
                    (*new_screen_span).end = (*screen_span).end;
                    (*new_screen_span).next = (*screen_span).next;

                    (*screen_span).next = new_screen_span;
                    (*screen_span).end = x0;

                    screen.valid_lines += 1;
                    break 'next_line;
                } else {
                    (*screen_span).end = x0;
                    prev_screen_link = ptr::addr_of_mut!((*screen_span).next);
                    screen_span = (*screen_span).next;
                    if screen_span.is_null() {
                        break 'next_line;
                    }
                }
            }

            // Spans entirely within the raster.
            while (*screen_span).end <= x1 {
                accept = 1;
                link_alloc(
                    a,
                    this,
                    &mut prev_link,
                    (*screen_span).start,
                    (*screen_span).end,
                )?;

                *prev_screen_link = (*screen_span).next;
                screen_span = (*screen_span).next;
                screen.valid_lines -= 1;
                if screen_span.is_null() {
                    break 'next_line;
                }
            }

            // Span overlaps the raster's end point.
            if (*screen_span).start < x1 {
                accept = 1;
                link_alloc(a, this, &mut prev_link, (*screen_span).start, x1)?;
                (*screen_span).start = x1;
            }
        }
        *prev_link = ptr::null_mut();
        line = line.add(1);
    }

    for _ in our_end..this.end_y {
        *temp = ptr::null_mut();
        temp = temp.add(1);
    }
    Ok(accept)
}

/// `FSpanBuffer::CopyFromRaster`: same walk as `copy_from_raster_update` but
/// `screen` is never modified. Returns 1 if the poly is all or partly visible.
///
/// # Safety
/// `mem` must be a live `FMemStack` and every span/index pointer reachable
/// from the buffers must be valid, as for the C++ caller.
pub unsafe fn copy_from_raster<A: SpanAlloc>(
    a: &A,
    this: &mut FSpanBuffer,
    screen: &FSpanBuffer,
    raster_start_y: i32,
    raster_end_y: i32,
    raster: *const FRasterSpan,
) -> Result<i32, i32> {
    let our_start = raster_start_y.max(screen.start_y);
    let our_end = raster_end_y.min(screen.end_y);
    let mut temp = this.index;

    if our_start >= our_end {
        for _ in this.start_y..this.end_y {
            *temp = ptr::null_mut();
            temp = temp.add(1);
        }
        return Ok(0);
    }

    for _ in this.start_y..our_start {
        *temp = ptr::null_mut();
        temp = temp.add(1);
    }

    let mut line = raster.add(count_of(our_start - raster_start_y));
    let mut screen_index = screen.index.add(count_of(our_start - screen.start_y));

    let mut accept = 0;
    for _ in our_start..our_end {
        let mut screen_span = *screen_index;
        screen_index = screen_index.add(1);
        let mut prev_link = temp;
        temp = temp.add(1);

        let x0 = (*line).x[0];
        let x1 = (*line).x[1];

        'next_line: {
            if screen_span.is_null() || x1 <= x0 {
                break 'next_line;
            }

            while (*screen_span).end <= x0 {
                screen_span = (*screen_span).next;
                if screen_span.is_null() {
                    break 'next_line;
                }
            }

            if (*screen_span).start < x0 {
                accept = 1;
                link_alloc(a, this, &mut prev_link, x0, x1.min((*screen_span).end))?;
                screen_span = (*screen_span).next;
                if screen_span.is_null() {
                    break 'next_line;
                }
            }

            while (*screen_span).end <= x1 {
                accept = 1;
                link_alloc(
                    a,
                    this,
                    &mut prev_link,
                    (*screen_span).start,
                    (*screen_span).end,
                )?;
                screen_span = (*screen_span).next;
                if screen_span.is_null() {
                    break 'next_line;
                }
            }

            if (*screen_span).start < x1 {
                accept = 1;
                link_alloc(a, this, &mut prev_link, (*screen_span).start, x1)?;
            }
        }
        *prev_link = ptr::null_mut();
        line = line.add(1);
    }

    for _ in our_end..this.end_y {
        *temp = ptr::null_mut();
        temp = temp.add(1);
    }
    Ok(accept)
}

/// `FSpanBuffer::MergeWith`: merge `other` into `this`, reallocating the
/// index (from `this.mem`) when `other`'s range is not inside `this`'s.
/// `Mark` is not changed.
///
/// # Safety
/// `mem` must be a live `FMemStack` and every span/index pointer reachable
/// from the buffers must be valid, as for the C++ caller.
pub unsafe fn merge_with<A: SpanAlloc>(
    a: &A,
    this: &mut FSpanBuffer,
    other: &FSpanBuffer,
) -> Result<(), i32> {
    if other.start_y < this.start_y || other.end_y > this.end_y {
        let new_start = this.start_y.min(other.start_y);
        let new_end = this.end_y.max(other.end_y);
        let new_num = count_of(new_end - new_start);
        let new_index = new_array::<A, *mut FSpan>(a, this.mem, new_num, DEFAULT_ALIGNMENT)?;

        let lead = count_of(this.start_y - new_start);
        let body = (this.end_y - this.start_y).max(0) as usize;
        let tail = count_of(new_end - this.end_y);
        ptr::write_bytes(new_index, 0, lead);
        if body > 0 {
            ptr::copy_nonoverlapping(this.index, new_index.add(lead), body);
        }
        ptr::write_bytes(new_index.add(new_num - tail), 0, tail);

        this.start_y = new_start;
        this.end_y = new_end;
        this.index = new_index;
    }

    // Now merge the other span buffer into this one.
    let mut this_index = this.index.add(count_of(other.start_y - this.start_y));
    let mut other_index = other.index;
    for _ in other.start_y..other.end_y {
        let mut prev_link = this_index;
        let mut this_span = *this_index;
        this_index = this_index.add(1);
        let mut other_span = *other_index;
        other_index = other_index.add(1);

        // Everything relative to this_span.
        while !this_span.is_null() && !other_span.is_null() {
            if (*other_span).end < (*this_span).start {
                // Link other_span in completely before this_span.
                let t = new_span(a, this.mem)?;
                *prev_link = t;
                (*t).start = (*other_span).start;
                (*t).end = (*other_span).end;
                (*t).next = this_span;
                prev_link = ptr::addr_of_mut!((*t).next);

                other_span = (*other_span).next;

                this.valid_lines += 1;
            } else if (*other_span).start <= (*this_span).end {
                // Merge other_span into this_span.
                *prev_link = this_span;
                (*this_span).start = (*this_span).start.min((*other_span).start);
                (*this_span).end = (*this_span).end.max((*other_span).end);
                let temp_span = this_span;

                prev_link = ptr::addr_of_mut!((*this_span).next);
                this_span = (*this_span).next;
                other_span = (*other_span).next;

                loop {
                    if !this_span.is_null() && (*this_span).start <= (*temp_span).end {
                        (*temp_span).end = (*this_span).end.max((*temp_span).end);
                        this_span = (*this_span).next;
                        this.valid_lines -= 1;
                    } else if !other_span.is_null() && (*other_span).start <= (*temp_span).end {
                        (*temp_span).end = (*temp_span).end.max((*other_span).end);
                        other_span = (*other_span).next;
                    } else {
                        break;
                    }
                }
            } else {
                // This span is entirely before the other span; keep it.
                *prev_link = this_span;
                prev_link = ptr::addr_of_mut!((*this_span).next);
                this_span = (*this_span).next;
            }
        }

        // Just append the remaining spans from other_span.
        while !other_span.is_null() {
            let t = new_span(a, this.mem)?;
            *prev_link = t;
            (*t).start = (*other_span).start;
            (*t).end = (*other_span).end;
            prev_link = ptr::addr_of_mut!((*t).next);

            other_span = (*other_span).next;

            this.valid_lines += 1;
        }
        *prev_link = this_span;
    }
    Ok(())
}

/// `FSpanBuffer::CopyIndexFrom`: copies `StartY`, `EndY` and the line-head
/// pointers (not `ValidLines`, not `Mem`, not `Mark`). The new index comes
/// from `mem`.
///
/// # Safety
/// `mem` must be a live `FMemStack` and every span/index pointer reachable
/// from the buffers must be valid, as for the C++ caller.
pub unsafe fn copy_index_from<A: SpanAlloc>(
    a: &A,
    this: &mut FSpanBuffer,
    source: &FSpanBuffer,
    mem: *mut c_void,
) -> Result<(), i32> {
    this.start_y = source.start_y;
    this.end_y = source.end_y;

    let n = (source.end_y - source.start_y).max(0) as usize;
    this.index = new_array::<A, *mut FSpan>(a, mem, n, DEFAULT_ALIGNMENT)?;
    if n > 0 {
        ptr::copy_nonoverlapping(source.index, this.index, n);
    }
    Ok(())
}
