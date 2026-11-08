//! Native-endian VM operands. Object/name operands are 32-bit indices, never
//! host pointers. Callers resolve indices through the existing storage seam.

/// Read an operand and publish the advanced native cursor before returning.
///
/// # Safety
/// `cursor` addresses a live writable code pointer, and that pointer addresses
/// enough initialized operand bytes for `T`. Only plain VM scalar/index types
/// may be used. No Rust reference to native frame storage may survive reentry.
#[inline]
pub unsafe fn read<T: Copy>(cursor: *mut *mut u8) -> T {
    unsafe {
        let code = cursor.read();
        let value = code.cast::<T>().read_unaligned();
        cursor.write(code.add(std::mem::size_of::<T>()));
        value
    }
}

/// # Safety
/// The live cursor and initialized operand bytes satisfy `read`'s contract.
#[inline]
pub unsafe fn peek<T: Copy>(cursor: *mut *mut u8) -> T {
    unsafe { cursor.read().cast::<T>().read_unaligned() }
}

/// # Safety
/// The adjusted cursor remains within the same live bytecode allocation, or
/// one byte past it. Negative adjustments require preceding readable code.
#[inline]
pub unsafe fn advance(cursor: *mut *mut u8, bytes: isize) {
    unsafe { cursor.write(cursor.read().offset(bytes)) }
}
