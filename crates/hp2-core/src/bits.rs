//! Port of the out-of-line bodies in `HarryPotter2/Unreal/Core/Src/UnBits.cpp`:
//! `appBitsCpy`, the `FBitWriter` and `FBitReader` members, and
//! `FBitWriterMark::Pop`. The C++ shells forward here through `hp2_rs_core_bits_*`
//! (`crates/hp2-ffi/src/core_bits.rs`). The inline members and the
//! `FBitWriterMark` fields stay in `UnBits.h`.
//!
//! Layouts come from the generated engine ABI (`hp2_abi::engine::generated`).
//! The `FArchive` base is not ported; its vptr is kept as padding and its
//! `i32` flag fields are read and written through the generated layout.
//! SDK bunches embed these archives with four-byte packing. Field projections
//! use unaligned loads/stores rather than imposing the standalone base alignment.
//!
//! Virtual calls: `FBitReader::Serialize` and `FBitReader::ReadInt` call
//! `SerializeBits`/`SerializeInt` virtually in C++. No class in the tree
//! overrides those (`FOutBunch`/`FInBunch` in `UnBunch.h` only inherit), so the
//! port calls the bodies directly.
//!
//! Rust owns buffer construction/replacement through the shared raw-array kernel.
//! The caller supplies its allocator so storage retains the engine's heap,
//! diagnostics and exception transport. [`Hp2BitsServices`] only retains the live
//! `checkSlow` failure leaf. `appMemzero` and `appMemcpy` are Rust helpers
//! (`memzero`, `memcpy`) that run only for counts `> 0`, keeping empty/null buffers
//! away from Rust pointer operations that require non-null pointers.
//!
//! Divergences from the original, documented where they occur:
//! - Signed `int` sums (`Num+LengthBits`, `DestBit+BitCount-1`, ...) use
//!   wrapping arithmetic, so overflow that is undefined in C++ wraps here.
//! - The legacy large-copy lead-out can read a byte beyond a byte-aligned source.
//!   The Rust kernel reuses its accumulator when the last legal byte has already
//!   been consumed, preserving copied bits without that unused read.
//! - `FBitWriterMark::Pop` with `Num > Writer.Num` (an invalid LIFO state) gives
//!   a negative zeroing size. The product calls `memset` with that count converted to `size_t`,
//!   which writes about 2^64 bytes: undefined behaviour. The port's `memzero` returns without
//!   writing for any count `<= 0`, so it does not replicate that write; it is not a faithful
//!   pass-through of the UB.

use std::ffi::c_void;
use std::ptr;

use crate::array_storage::{self, Hp2ArrayServices, Hp2ArraySlots};
use hp2_abi::engine::generated::{FBitReader, FBitWriter, TArrayRaw};

/// Returned when a C++ callback raised an exception. The port stops at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Threw;

/// `GShift` in `UnBits.cpp`.
const GSHIFT: [u8; 8] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80];
/// `GMask` in `UnBits.cpp`.
const GMASK: [u8; 8] = [0x00, 0x01, 0x03, 0x07, 0x0f, 0x1f, 0x3f, 0x7f];
const EMPTY_TAG: [u32; 1] = [0];

/// `appCeilLogTwo(Arg)`: the smallest `n` for which `2^n >= Arg`.
/// The legacy function returns zero for zero as well as one.
pub fn ceil_log_two(arg: u32) -> u8 {
    if arg == 0 {
        0
    } else {
        (u32::BITS - (arg - 1).leading_zeros()) as u8
    }
}

/// Original line of the `checkSlow(Value<ValueMax)` in `FBitWriter::SerializeInt`.
const LINE_SERIALIZE_INT: i32 = 168;
/// Original line of the `checkSlow(Value<ValueMax)` in `FBitWriter::WriteInt`.
const LINE_WRITE_INT: i32 = 191;
/// Original line of the `checkSlow(Num<=Writer.Num)` in `FBitWriterMark::Pop`.
const LINE_POP_NUM: i32 = 250;
/// Original line of the `checkSlow(Num<=Writer.Max)` in `FBitWriterMark::Pop`.
const LINE_POP_MAX: i32 = 251;

/// Native assertion policy. Buffer ownership uses `Hp2ArrayServices` directly,
/// without byte-array or reader-dispatch callbacks.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2BitsServices {
    /// `DO_GUARD_SLOW`: nonzero when the `checkSlow` tests are live.
    pub do_guard_slow: i32,
    /// `appFailAssert(Expr, __FILE__, Line)`. `expr` is NUL-terminated. The thunk
    /// is `Hp2BitsCheckFail` in `UnBits.cpp`, so `__FILE__` is the original TU.
    pub check_fail: unsafe extern "C" fn(expr: *const c_void, line: i32) -> i32,
}

/// Maps a status from a C++ callback to the port's result.
fn raised(status: i32) -> Result<(), Threw> {
    if status != 0 { Err(Threw) } else { Ok(()) }
}

unsafe fn array_slots(array: *mut TArrayRaw) -> Hp2ArraySlots {
    unsafe {
        Hp2ArraySlots {
            data: ptr::addr_of_mut!((*array).data),
            num: ptr::addr_of_mut!((*array).num),
            max: ptr::addr_of_mut!((*array).max),
        }
    }
}

/// `checkSlow(Expr)` failure in the live build. Returns `Err(Threw)` when the C++
/// side raised. The caller tests the condition first, so nothing runs when
/// `DO_GUARD_SLOW` is off.
fn check_fail(svc: &Hp2BitsServices, expr: &[u8], line: i32) -> Result<(), Threw> {
    debug_assert_eq!(expr.last(), Some(&0));
    // SAFETY: `expr` is NUL-terminated static text. The table is the C++ side's.
    raised(unsafe { (svc.check_fail)(expr.as_ptr().cast(), line) })
}

/// `&Buffer(Index)` for a `TArray<BYTE>` whose data pointer is `data`. Unchecked,
/// as the product build's `operator()` is.
#[inline]
fn byte_at(data: *mut c_void, index: i32) -> *mut u8 {
    data.cast::<u8>().wrapping_offset(index as isize)
}

/// `appMemzero(Dest, Count)`: zeroes `count` bytes. A count `<= 0` is a no-op;
/// the product's negative-count `memset` is undefined and is not replicated.
#[inline]
unsafe fn memzero(dest: *mut c_void, count: i32) {
    if count > 0 {
        // SAFETY: caller contract: `dest` is writable for `count` bytes.
        unsafe { ptr::write_bytes(dest.cast::<u8>(), 0, count as usize) };
    }
}

/// `appMemcpy(Dest, Src, Count)`: copies `count` bytes. A count `<= 0` is a no-op.
/// Callers pass non-overlapping buffers: the writer or reader buffer and the caller's source.
#[inline]
unsafe fn memcpy(dest: *mut c_void, src: *const c_void, count: i32) {
    if count > 0 {
        // SAFETY: caller contract: `src` is readable and `dest` writable for `count` bytes, non-overlapping.
        unsafe { ptr::copy_nonoverlapping(src.cast::<u8>(), dest.cast::<u8>(), count as usize) };
    }
}

/// `appBitsCpy`: copies `bit_count` bits from `src` at bit `src_bit` to `dest` at
/// bit `dest_bit`. Bit numbering is LSB-first within each byte.
///
/// # Safety
/// `dest` and `src` must cover the bytes used by their requested bit ranges.
/// A null pointer is valid for a zero-bit copy, which does not access storage.
pub unsafe fn bits_cpy(dest: *mut u8, dest_bit: i32, src: *const u8, src_bit: i32, bit_count: i32) {
    if bit_count == 0 {
        return;
    }
    // SAFETY: forwarded from the caller's contract.
    unsafe {
        if bit_count <= 8 {
            cpy_small(dest, dest_bit, src, src_bit, bit_count);
        } else {
            cpy_large(dest, dest_bit, src, src_bit, bit_count);
        }
    }
}

/// `appBitsCpy` for `BitCount <= 8`. Only the bytes the bits use are touched.
///
/// # Safety
/// As [`bits_cpy`].
unsafe fn cpy_small(dest: *mut u8, dest_bit: i32, src: *const u8, src_bit: i32, bit_count: i32) {
    let dest_end = dest_bit.wrapping_add(bit_count).wrapping_sub(1);
    let src_end = src_bit.wrapping_add(bit_count).wrapping_sub(1);
    let dest_index = (dest_bit / 8) as u32;
    let src_index = (src_bit / 8) as u32;
    let last_dest = (dest_end / 8) as u32;
    let last_src = (src_end / 8) as u32;
    let shift_src = (src_bit & 7) as u32;
    let shift_dest = (dest_bit & 7) as u32;
    let first_mask: u32 = 0xFF << shift_dest;
    // Pre-shifted left by 1.
    let last_mask: u32 = 0xFE << ((dest_end & 7) as u32);

    // SAFETY: the same bytes the original reads and writes.
    unsafe {
        let src_byte = u32::from(*src.wrapping_add(src_index as usize));
        let accu: u32 = if src_index == last_src {
            src_byte >> shift_src
        } else {
            (src_byte >> shift_src)
                | (u32::from(*src.wrapping_add(last_src as usize)) << (8 - shift_src))
        };

        let d_index = dest.wrapping_add(dest_index as usize);
        if dest_index == last_dest {
            let multi_mask = first_mask & !last_mask;
            *d_index =
                ((u32::from(*d_index) & !multi_mask) | ((accu << shift_dest) & multi_mask)) as u8;
        } else {
            *d_index =
                ((u32::from(*d_index) & !first_mask) | ((accu << shift_dest) & first_mask)) as u8;
            let d_last = dest.wrapping_add(last_dest as usize);
            *d_last = ((u32::from(*d_last) & last_mask) | ((accu >> (8 - shift_dest)) & !last_mask))
                as u8;
        }
    }
}

/// `appBitsCpy` for `BitCount > 8`. Byte-sized shifting with a lead-in, a fast
/// inner loop, and a lead-out.
///
/// # Safety
/// As [`bits_cpy`].
unsafe fn cpy_large(dest: *mut u8, dest_bit: i32, src: *const u8, src_bit: i32, bit_count: i32) {
    let dest_end = dest_bit.wrapping_add(bit_count);
    let src_end = src_bit.wrapping_add(bit_count);
    let mut dest_index = (dest_bit / 8) as u32;
    let first_src_mask: u32 = 0xFF << ((dest_bit & 7) as u32);
    let last_dest = (dest_end / 8) as u32;
    let last_src_mask: u32 = 0xFF << ((dest_end & 7) as u32);
    let mut src_index = (src_bit / 8) as u32;
    let last_src = (src_end / 8) as u32;
    let mut shift_count: i32 = (dest_bit & 7) - (src_bit & 7);
    let dest_loop = last_dest.wrapping_sub(dest_index) as i32;
    let src_loop = last_src.wrapping_sub(src_index) as i32;
    let mut full_loop: u32;
    let mut bit_accu: u32;

    // SAFETY: the same bytes the original reads and writes.
    unsafe {
        // Lead-in needs to read one or two source bytes depending on alignment.
        if shift_count >= 0 {
            full_loop = dest_loop.max(src_loop) as u32;
            bit_accu = u32::from(*src.wrapping_add(src_index as usize)) << (shift_count as u32);
            shift_count += 8; // Prepare for the inner loop.
        } else {
            shift_count += 8; // Turn shifts -7..-1 into +1..+7.
            full_loop = dest_loop.max(src_loop.wrapping_sub(1)) as u32;
            bit_accu = u32::from(*src.wrapping_add(src_index as usize)) << (shift_count as u32);
            src_index = src_index.wrapping_add(1);
            shift_count += 8; // Prepare for the inner loop.
            bit_accu = (u32::from(*src.wrapping_add(src_index as usize)) << (shift_count as u32))
                .wrapping_add(bit_accu)
                >> 8;
        }

        // Lead-in: first copy.
        let d_first = dest.wrapping_add(dest_index as usize);
        *d_first = ((bit_accu & first_src_mask) | (u32::from(*d_first) & !first_src_mask)) as u8;
        src_index = src_index.wrapping_add(1);
        dest_index = dest_index.wrapping_add(1);

        // Fast inner loop. `shift_count` is 8..=15, so every read is relevant.
        while full_loop > 1 {
            bit_accu = (u32::from(*src.wrapping_add(src_index as usize)) << (shift_count as u32))
                .wrapping_add(bit_accu)
                >> 8;
            src_index = src_index.wrapping_add(1);
            *dest.wrapping_add(dest_index as usize) = bit_accu as u8;
            dest_index = dest_index.wrapping_add(1);
            full_loop = full_loop.wrapping_sub(1);
        }

        // Lead-out.
        if last_src_mask != 0xFF {
            let last_legal_src = (src_end.wrapping_sub(1) as u32) / 8;
            if last_legal_src == src_index {
                bit_accu = (u32::from(*src.wrapping_add(src_index as usize))
                    << (shift_count as u32))
                    .wrapping_add(bit_accu)
                    >> 8;
            } else {
                bit_accu >>= 8;
            }
            let d_last = dest.wrapping_add(dest_index as usize);
            *d_last = ((u32::from(*d_last) & last_src_mask) | (bit_accu & !last_src_mask)) as u8;
        }
    }
}

/// Construct a writer's buffer and initialize its archive flags.
///
/// # Safety
/// `w` must point to a live `FBitWriter` with an initialized empty buffer.
/// `arrays` must use the allocator that will release that buffer, and its
/// callbacks must preserve the live slots through reentry without unwinding.
pub unsafe fn writer_init(
    w: *mut FBitWriter,
    max_bits: i32,
    arrays: Hp2ArrayServices,
) -> Result<(), Threw> {
    // SAFETY: caller contract. Exact construction preserves the native capacity,
    // including the zero-count allocation call.
    unsafe {
        ptr::addr_of!((*w).num).cast_mut().write_unaligned(0);
        ptr::addr_of!((*w).max).cast_mut().write_unaligned(max_bits);
        let slots = array_slots(ptr::addr_of_mut!((*w).buffer));
        array_storage::set_exact(
            slots, max_bits.wrapping_add(7) >> 3, 1, EMPTY_TAG.as_ptr().cast(), arrays,
        ).map_err(|_| Threw)?;
        memzero(ptr::addr_of!((*w).buffer.data).read_unaligned(), ptr::addr_of!((*w).buffer.num).read_unaligned());
        ptr::addr_of!((*w).ar_is_persistent).cast_mut().write_unaligned(1);
        ptr::addr_of!((*w).ar_is_saving).cast_mut().write_unaligned(1);
        ptr::addr_of!((*w).ar_net_ver).cast_mut().write_unaligned(ptr::addr_of!((*w).ar_net_ver).read_unaligned() | i32::MIN); // 0x80000000
    }
    Ok(())
}

/// `FBitWriter::SerializeBits(Src, LengthBits)` body.
///
/// # Safety
/// `w` must point to a live `FBitWriter`. `src` must be readable for
/// `LengthBits` bits (one byte when `LengthBits == 1`), as the original reads it.
pub unsafe fn writer_serialize_bits(w: *mut FBitWriter, src: *const u8, length_bits: i32) {
    // SAFETY: caller contract.
    unsafe {
        let num = ptr::addr_of!((*w).num).read_unaligned();
        if num.wrapping_add(length_bits) <= ptr::addr_of!((*w).max).read_unaligned() {
            if length_bits == 1 {
                if (*src & 0x01) != 0 {
                    let p = byte_at(ptr::addr_of!((*w).buffer.data).read_unaligned(), num >> 3);
                    *p |= GSHIFT[(num & 7) as usize];
                }
                ptr::addr_of!((*w).num).cast_mut().write_unaligned(num.wrapping_add(1));
            } else {
                bits_cpy(ptr::addr_of!((*w).buffer.data).read_unaligned().cast(), num, src, 0, length_bits);
                ptr::addr_of!((*w).num).cast_mut().write_unaligned(num.wrapping_add(length_bits));
            }
        } else {
            ptr::addr_of!((*w).ar_is_error).cast_mut().write_unaligned(1);
        }
    }
}

/// `FBitWriter::Serialize(Src, LengthBytes)` body.
///
/// # Safety
/// As [`writer_serialize_bits`], for `LengthBytes * 8` bits.
pub unsafe fn writer_serialize(w: *mut FBitWriter, src: *const u8, length_bytes: i32) {
    // SAFETY: caller contract.
    unsafe {
        let length_bits = length_bytes.wrapping_mul(8);
        if ptr::addr_of!((*w).num).read_unaligned().wrapping_add(length_bits) <= ptr::addr_of!((*w).max).read_unaligned() {
            bits_cpy(ptr::addr_of!((*w).buffer.data).read_unaligned().cast(), ptr::addr_of!((*w).num).read_unaligned(), src, 0, length_bits);
            ptr::addr_of!((*w).num).cast_mut().write_unaligned(ptr::addr_of!((*w).num).read_unaligned().wrapping_add(length_bits));
        } else {
            ptr::addr_of!((*w).ar_is_error).cast_mut().write_unaligned(1);
        }
    }
}

/// Shared `SerializeInt`/`WriteInt` body with a caller-owned assertion policy.
///
/// # Safety
/// `w` must point to a live `FBitWriter` whose buffer covers `(max+7)>>3` bytes.
unsafe fn write_int_with_checker<E>(
    w: *mut FBitWriter,
    value: u32,
    value_max: u32,
    do_guard_slow: i32,
    check_line: i32,
    mut check_failed: impl FnMut(&[u8], i32) -> Result<(), E>,
) -> Result<(), E> {
    if do_guard_slow != 0 && !(value < value_max) {
        check_failed(b"Value<ValueMax\0", check_line)?;
    }
    // SAFETY: caller contract.
    unsafe {
        let bits = ceil_log_two(value_max);
        if ptr::addr_of!((*w).num).read_unaligned().wrapping_add(i32::from(bits)) <= ptr::addr_of!((*w).max).read_unaligned() {
            let mut new_value: u32 = 0;
            let mut mask: u32 = 1;
            let mut num = ptr::addr_of!((*w).num).read_unaligned();
            let data = ptr::addr_of!((*w).buffer.data).read_unaligned();
            while new_value.wrapping_add(mask) < value_max && mask != 0 {
                if (value & mask) != 0 {
                    let p = byte_at(data, num >> 3);
                    *p = (*p).wrapping_add(GSHIFT[(num & 7) as usize]);
                    new_value = new_value.wrapping_add(mask);
                }
                mask = mask.wrapping_mul(2);
                num = num.wrapping_add(1);
            }
            ptr::addr_of!((*w).num).cast_mut().write_unaligned(num);
        } else {
            ptr::addr_of!((*w).ar_is_error).cast_mut().write_unaligned(1);
        }
    }
    Ok(())
}

unsafe fn write_int(
    w: *mut FBitWriter,
    value: u32,
    value_max: u32,
    svc: &Hp2BitsServices,
    check_line: i32,
) -> Result<(), Threw> {
    // SAFETY: forwarded to the shared body; the service outlives this call.
    unsafe {
        write_int_with_checker(w, value, value_max, svc.do_guard_slow, check_line,
            |expr, line| check_fail(svc, expr, line))
    }
}

/// Core caller variant of `FBitWriter::WriteInt`, with its own check-error policy.
///
/// # Safety
/// `w` must point to a live `FBitWriter` whose buffer covers `(max+7)>>3` bytes.
pub unsafe fn writer_write_int_with_checker<E>(
    w: *mut FBitWriter,
    value: u32,
    value_max: u32,
    do_guard_slow: i32,
    check_failed: impl FnMut(&[u8], i32) -> Result<(), E>,
) -> Result<(), E> {
    // SAFETY: forwarded.
    unsafe { write_int_with_checker(w, value, value_max, do_guard_slow, LINE_WRITE_INT, check_failed) }
}

/// `FBitWriter::SerializeInt(Value, ValueMax)` body. `Value` is only read.
///
/// # Safety
/// As [`write_int`].
pub unsafe fn writer_serialize_int(
    w: *mut FBitWriter,
    value: u32,
    value_max: u32,
    svc: &Hp2BitsServices,
) -> Result<(), Threw> {
    // SAFETY: forwarded.
    unsafe { write_int(w, value, value_max, svc, LINE_SERIALIZE_INT) }
}

/// `FBitWriter::WriteInt(Value, ValueMax)` body. Identical to
/// [`writer_serialize_int`] apart from the `checkSlow` line.
///
/// # Safety
/// As [`write_int`].
pub unsafe fn writer_write_int(
    w: *mut FBitWriter,
    value: u32,
    value_max: u32,
    svc: &Hp2BitsServices,
) -> Result<(), Threw> {
    // SAFETY: forwarded.
    unsafe { write_int(w, value, value_max, svc, LINE_WRITE_INT) }
}

/// `FBitWriter::WriteBit(In)` body.
///
/// # Safety
/// `w` must point to a live `FBitWriter` whose buffer covers `(max+7)>>3` bytes.
pub unsafe fn writer_write_bit(w: *mut FBitWriter, bit: u8) {
    // SAFETY: caller contract.
    unsafe {
        let num = ptr::addr_of!((*w).num).read_unaligned();
        if num.wrapping_add(1) <= ptr::addr_of!((*w).max).read_unaligned() {
            if bit != 0 {
                let p = byte_at(ptr::addr_of!((*w).buffer.data).read_unaligned(), num >> 3);
                *p |= GSHIFT[(num & 7) as usize];
            }
            ptr::addr_of!((*w).num).cast_mut().write_unaligned(num.wrapping_add(1));
        } else {
            ptr::addr_of!((*w).ar_is_error).cast_mut().write_unaligned(1);
        }
    }
}

/// `FBitWriter::GetData()` body.
///
/// # Safety
/// `w` must point to a live `FBitWriter`.
pub unsafe fn writer_get_data(w: *mut FBitWriter) -> *mut u8 {
    // SAFETY: caller contract.
    unsafe { byte_at(ptr::addr_of!((*w).buffer.data).read_unaligned(), 0) }
}

/// `FBitWriter::GetNumBytes()` body.
///
/// # Safety
/// `w` must point to a live `FBitWriter`.
pub unsafe fn writer_get_num_bytes(w: *mut FBitWriter) -> i32 {
    // SAFETY: caller contract.
    unsafe { ptr::addr_of!((*w).num).read_unaligned().wrapping_add(7) >> 3 }
}

/// `FBitWriter::GetNumBits()` body.
///
/// # Safety
/// `w` must point to a live `FBitWriter`.
pub unsafe fn writer_get_num_bits(w: *mut FBitWriter) -> i32 {
    // SAFETY: caller contract.
    unsafe { ptr::addr_of!((*w).num).read_unaligned() }
}

/// `FBitWriter::SetOverflowed()` body.
///
/// # Safety
/// `w` must point to a live `FBitWriter`.
pub unsafe fn writer_set_overflowed(w: *mut FBitWriter) {
    // SAFETY: caller contract.
    unsafe {
        ptr::addr_of!((*w).ar_is_error).cast_mut().write_unaligned(1);
    }
}

/// `FBitWriterMark::Pop(Writer)` body. `overflowed` and `num` are the mark's
/// `Overflowed` and `Num` members.
///
/// # Safety
/// `w` must point to a live `FBitWriter` whose buffer covers `(max+7)>>3` bytes.
pub unsafe fn writer_mark_pop(
    w: *mut FBitWriter,
    overflowed: i32,
    num: i32,
    svc: &Hp2BitsServices,
) -> Result<(), Threw> {
    // SAFETY: the caller of this compatibility wrapper upholds the writer contract.
    unsafe {
        writer_mark_pop_with_checker(w, overflowed, num, |expr, line| {
            if svc.do_guard_slow != 0 { check_fail(svc, expr, line) } else { Ok(()) }
        })
    }
}

/// Pops a writer mark using the caller's assertion policy.
///
/// # Safety
/// `w` must point to a live `FBitWriter` whose buffer covers `(max+7)>>3` bytes.
pub unsafe fn writer_mark_pop_with_checker<E>(
    w: *mut FBitWriter,
    overflowed: i32,
    num: i32,
    mut check: impl FnMut(&[u8], i32) -> Result<(), E>,
) -> Result<(), E> {
    // SAFETY: caller contract.
    unsafe {
        if !(num <= ptr::addr_of!((*w).num).read_unaligned()) {
            check(b"Num<=Writer.Num\0", LINE_POP_NUM)?;
        }
        if !(num <= ptr::addr_of!((*w).max).read_unaligned()) {
            check(b"Num<=Writer.Max\0", LINE_POP_MAX)?;
        }

        let data = ptr::addr_of!((*w).buffer.data).read_unaligned();
        if (num & 7) != 0 {
            let p = byte_at(data, num >> 3);
            *p &= GMASK[(num & 7) as usize];
        }
        let start = num.wrapping_add(7) >> 3;
        let end = ptr::addr_of!((*w).num).read_unaligned().wrapping_add(7) >> 3;
        memzero(byte_at(data, start).cast(), end.wrapping_sub(start));

        ptr::addr_of!((*w).ar_is_error).cast_mut().write_unaligned(overflowed);
        ptr::addr_of!((*w).num).cast_mut().write_unaligned(num);
    }
    Ok(())
}

/// Construct a reader's buffer and initialize its archive flags.
///
/// # Safety
/// `r` must point to a live `FBitReader` with an initialized empty buffer.
/// A non-null `src` must be readable for `(count_bits+7)>>3` bytes.
/// `arrays` must use the allocator that will release that buffer, and its
/// callbacks must preserve the live slots through reentry without unwinding.
pub unsafe fn reader_init(
    r: *mut FBitReader,
    src: *const u8,
    count_bits: i32,
    arrays: Hp2ArrayServices,
) -> Result<(), Threw> {
    // SAFETY: caller contract.
    unsafe {
        ptr::addr_of!((*r).num).cast_mut().write_unaligned(count_bits);
        ptr::addr_of!((*r).pos).cast_mut().write_unaligned(0);
        let slots = array_slots(ptr::addr_of_mut!((*r).buffer));
        array_storage::set_exact(
            slots, count_bits.wrapping_add(7) >> 3, 1, EMPTY_TAG.as_ptr().cast(), arrays,
        ).map_err(|_| Threw)?;
        ptr::addr_of!((*r).ar_is_persistent).cast_mut().write_unaligned(1);
        ptr::addr_of!((*r).ar_is_loading).cast_mut().write_unaligned(1);
        ptr::addr_of!((*r).ar_net_ver).cast_mut().write_unaligned(ptr::addr_of!((*r).ar_net_ver).read_unaligned() | i32::MIN); // 0x80000000
        if !src.is_null() {
            memcpy(
                ptr::addr_of!((*r).buffer.data).read_unaligned(),
                src.cast(),
                count_bits.wrapping_add(7) >> 3,
            );
        }
    }
    Ok(())
}

/// `FBitReader::SetData(Src, CountBits)` body.
///
/// # Safety
/// `r` and `src` must point to live `FBitReader`s. `arrays` must pair with
/// the destination buffer's allocator and capture exceptions without unwinding.
pub unsafe fn reader_set_data(
    r: *mut FBitReader,
    src: *mut FBitReader,
    count_bits: i32,
    arrays: Hp2ArrayServices,
) -> Result<(), Threw> {
    // SAFETY: caller contract.
    unsafe {
        ptr::addr_of!((*r).num).cast_mut().write_unaligned(count_bits);
        ptr::addr_of!((*r).pos).cast_mut().write_unaligned(0);
        ptr::addr_of!((*r).ar_is_error).cast_mut().write_unaligned(0);
        let slots = array_slots(ptr::addr_of_mut!((*r).buffer));
        array_storage::empty(slots, 0, 1, EMPTY_TAG.as_ptr().cast(), arrays)
            .map_err(|_| Threw)?;
        array_storage::add(
            slots, count_bits.wrapping_add(7) >> 3, 1, EMPTY_TAG.as_ptr().cast(), arrays,
        ).map_err(|_| Threw)?;
        reader_serialize_bits(src, ptr::addr_of!((*r).buffer.data).read_unaligned().cast(), count_bits);
    }
    Ok(())
}

/// `FBitReader::SerializeBits(Dest, LengthBits)` body.
///
/// # Safety
/// `r` must point to a live `FBitReader`. `dest` must be writable for
/// `(length_bits+7)>>3` bytes (one byte when `length_bits == 1`).
pub unsafe fn reader_serialize_bits(r: *mut FBitReader, dest: *mut u8, length_bits: i32) {
    // SAFETY: caller contract.
    unsafe {
        let pos = ptr::addr_of!((*r).pos).read_unaligned();
        if pos.wrapping_add(length_bits) <= ptr::addr_of!((*r).num).read_unaligned() {
            let data = ptr::addr_of!((*r).buffer.data).read_unaligned();
            if length_bits == 1 {
                let bit = (*byte_at(data, pos >> 3) & GSHIFT[(pos & 7) as usize]) != 0;
                *dest = u8::from(bit);
                ptr::addr_of!((*r).pos).cast_mut().write_unaligned(pos.wrapping_add(1));
            } else {
                memzero(dest.cast(), length_bits.wrapping_add(7) >> 3);
                bits_cpy(dest, 0, data.cast(), pos, length_bits);
                ptr::addr_of!((*r).pos).cast_mut().write_unaligned(pos.wrapping_add(length_bits));
            }
        } else {
            ptr::addr_of!((*r).ar_is_error).cast_mut().write_unaligned(1);
        }
    }
}

/// `FBitReader::SerializeInt(Value, ValueMax)` body. `*value` is written on every
/// bit as the original does, so a partial value survives an overflow.
///
/// # Safety
/// `r` must point to a live `FBitReader`. `value` must be writable.
pub unsafe fn reader_serialize_int(r: *mut FBitReader, value: *mut u32, value_max: u32) {
    // SAFETY: caller contract.
    unsafe {
        *value = 0;
        let data = ptr::addr_of!((*r).buffer.data).read_unaligned();
        let num = ptr::addr_of!((*r).num).read_unaligned();
        let mut pos = ptr::addr_of!((*r).pos).read_unaligned();
        let mut mask: u32 = 1;
        while (*value).wrapping_add(mask) < value_max && mask != 0 {
            if pos >= num {
                ptr::addr_of!((*r).ar_is_error).cast_mut().write_unaligned(1);
                break;
            }
            if (*byte_at(data, pos >> 3) & GSHIFT[(pos & 7) as usize]) != 0 {
                *value |= mask;
            }
            mask = mask.wrapping_mul(2);
            pos = pos.wrapping_add(1);
        }
        ptr::addr_of!((*r).pos).cast_mut().write_unaligned(pos);
    }
}

/// `FBitReader::ReadInt(Max)` body.
///
/// # Safety
/// As [`reader_serialize_int`].
pub unsafe fn reader_read_int(r: *mut FBitReader, max: u32) -> u32 {
    let mut value = 0u32;
    // SAFETY: forwarded; `value` is a live local.
    unsafe { reader_serialize_int(r, &mut value, max) };
    value
}

/// `FBitReader::ReadBit()` body.
///
/// # Safety
/// `r` must point to a live `FBitReader`.
pub unsafe fn reader_read_bit(r: *mut FBitReader) -> u8 {
    // SAFETY: caller contract.
    unsafe {
        let old_pos = ptr::addr_of!((*r).pos).read_unaligned();
        ptr::addr_of!((*r).pos).cast_mut().write_unaligned(old_pos.wrapping_add(1));
        if ptr::addr_of!((*r).pos).read_unaligned() > ptr::addr_of!((*r).num).read_unaligned() {
            ptr::addr_of!((*r).ar_is_error).cast_mut().write_unaligned(1);
            0
        } else {
            u8::from(
                (*byte_at(ptr::addr_of!((*r).buffer.data).read_unaligned(), old_pos >> 3) & GSHIFT[(old_pos & 7) as usize]) != 0,
            )
        }
    }
}

/// `FBitReader::Serialize(Dest, LengthBytes)` body.
///
/// # Safety
/// As [`reader_serialize_bits`], for `length_bytes * 8` bits.
pub unsafe fn reader_serialize(r: *mut FBitReader, dest: *mut u8, length_bytes: i32) {
    // SAFETY: forwarded.
    unsafe { reader_serialize_bits(r, dest, length_bytes.wrapping_mul(8)) }
}

/// `FBitReader::GetData()` body.
///
/// # Safety
/// `r` must point to a live `FBitReader`.
pub unsafe fn reader_get_data(r: *mut FBitReader) -> *mut u8 {
    // SAFETY: caller contract.
    unsafe { byte_at(ptr::addr_of!((*r).buffer.data).read_unaligned(), 0) }
}

/// `FBitReader::AtEnd()` body. Returns `UBOOL`.
///
/// # Safety
/// `r` must point to a live `FBitReader`.
pub unsafe fn reader_at_end(r: *mut FBitReader) -> i32 {
    // SAFETY: caller contract.
    unsafe { i32::from(ptr::addr_of!((*r).ar_is_error).read_unaligned() != 0 || ptr::addr_of!((*r).pos).read_unaligned() == ptr::addr_of!((*r).num).read_unaligned()) }
}

/// `FBitReader::SetOverflowed()` body.
///
/// # Safety
/// `r` must point to a live `FBitReader`.
pub unsafe fn reader_set_overflowed(r: *mut FBitReader) {
    // SAFETY: caller contract.
    unsafe {
        ptr::addr_of!((*r).ar_is_error).cast_mut().write_unaligned(1);
    }
}

/// `FBitReader::GetNumBytes()` body.
///
/// # Safety
/// `r` must point to a live `FBitReader`.
pub unsafe fn reader_get_num_bytes(r: *mut FBitReader) -> i32 {
    // SAFETY: caller contract.
    unsafe { ptr::addr_of!((*r).num).read_unaligned().wrapping_add(7) >> 3 }
}

/// `FBitReader::GetNumBits()` body.
///
/// # Safety
/// `r` must point to a live `FBitReader`.
pub unsafe fn reader_get_num_bits(r: *mut FBitReader) -> i32 {
    // SAFETY: caller contract.
    unsafe { ptr::addr_of!((*r).num).read_unaligned() }
}

/// `FBitReader::GetPosBits()` body.
///
/// # Safety
/// `r` must point to a live `FBitReader`.
pub unsafe fn reader_get_pos_bits(r: *mut FBitReader) -> i32 {
    // SAFETY: caller contract.
    unsafe { ptr::addr_of!((*r).pos).read_unaligned() }
}
