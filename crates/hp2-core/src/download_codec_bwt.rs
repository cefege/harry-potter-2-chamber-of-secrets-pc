//! The original clamped-suffix BWT wire policy over live native archives.
//!
//! Scratch allocations are deliberately not cleared between blocks: an archive
//! can leave an already initialized tail untouched, and output Serialize can
//! mutate the actual headers and bytes used later in the invocation.
//!
//! As in the native implementation, callers must not cause uninitialized reads,
//! out-of-allocation accesses or signed-overflow arithmetic. In particular, the
//! native decode limit check admits MAX_BUFFER_SIZE + 1 before incrementing the
//! length; that value overruns its allocation and is not a supported native
//! behavior. Likewise, unwritten first-read bytes and corruption of the original
//! static compression pointers by reentry are not defined compatibility cases.

use crate::download_codec::{CodecIo, CodecResult, Hp2CodecFailure};
use std::mem::MaybeUninit;

const MAX_BUFFER_SIZE: usize = 0x40000;
const POSITION_COUNT: usize = MAX_BUFFER_SIZE + 1;

// Keeping these vectors at length zero reserves storage without initializing it.
// The raw pointers remain valid for their capacities, and only initialized
// elements are read under the native archive's contract.
fn scratch<T>(count: usize) -> Vec<MaybeUninit<T>> {
    Vec::with_capacity(count)
}

/// Sort suffixes with a terminator greater than every byte. This is precisely
/// ClampedBufferCompare: when the overlapping bytes match, the earlier starting
/// position (the longer suffix) sorts first, including the empty suffix last.
/// Prefix doubling avoids repeatedly rescanning long equal byte runs. Both rank
/// arrays and the position array are reused by the next block.
unsafe fn sort_suffixes(
    buffer: *const u8,
    positions: *mut i32,
    mut ranks: *mut i32,
    mut next_ranks: *mut i32,
    length: i32,
) {
    // SAFETY: the caller supplies initialized input bytes and scratch capacity
    // for every suffix; each rank generation is fully written before use.
    unsafe {
        let count = length.wrapping_add(1) as usize;
        for index in 0..count {
            positions.add(index).write(index as i32);
            ranks.add(index).write(if index == length as usize {
                256
            } else {
                i32::from(buffer.add(index).read())
            });
        }
        if count <= 1 {
            return;
        }

        let positions = std::slice::from_raw_parts_mut(positions, count);
        let mut span = 1;
        loop {
            // Beyond the terminator the rank is immaterial once a suffix is unique;
            // using the maximal value also preserves the shorter-suffix-last rule.
            let key = |position: i32| {
                let index = position as usize;
                (
                    ranks.add(index).read(),
                    if index + span < count {
                        ranks.add(index + span).read()
                    } else {
                        i32::MAX
                    },
                )
            };
            positions.sort_unstable_by_key(|&position| key(position));
            let mut rank = 0;
            next_ranks.add(positions[0] as usize).write(rank);
            for index in 1..count {
                if key(positions[index - 1]) != key(positions[index]) {
                    rank += 1;
                }
                next_ranks.add(positions[index] as usize).write(rank);
            }
            if rank as usize == count - 1 {
                return;
            }
            std::mem::swap(&mut ranks, &mut next_ranks);
            span *= 2;
        }
    }
}

/// # Safety
/// The services and archives must satisfy CodecIo's contract, and their reads
/// and mutations must stay within the original implementation's defined domain
/// described above. Output callbacks may mutate the supplied live storage.
pub unsafe fn encode(io: &CodecIo<'_>) -> CodecResult<i32> {
    // SAFETY: the archive contract establishes initialized reads and valid
    // mutations; invocation-local scratch remains allocated across callbacks.
    unsafe {
        let mut buffer_storage = scratch::<u8>(MAX_BUFFER_SIZE);
        let mut position_storage = scratch::<i32>(POSITION_COUNT);
        let mut rank_storage = scratch::<i32>(POSITION_COUNT);
        let mut next_rank_storage = scratch::<i32>(POSITION_COUNT);
        let buffer = buffer_storage.as_mut_ptr().cast::<u8>();
        let positions = position_storage.as_mut_ptr().cast::<i32>();
        let ranks = rank_storage.as_mut_ptr().cast::<i32>();
        let next_ranks = next_rank_storage.as_mut_ptr().cast::<i32>();
        let mut length: i32;
        let mut first = 0_i32;
        let mut last = 0_i32;

        while !io.at_end()? {
            let total_size = io.total_size()?;
            let position = io.tell()?;
            length = total_size
                .wrapping_sub(position)
                .min(MAX_BUFFER_SIZE as i32);
            io.read(buffer, length)?;
            sort_suffixes(buffer, positions, ranks, next_ranks, length);
            let mut index = 0_i32;
            while index < length.wrapping_add(1) {
                io.check_value(
                    Hp2CodecFailure::ArrayIndex,
                    (index as u32) < POSITION_COUNT as u32,
                    index,
                )?;
                let position = positions.add(index as usize).read();
                if position == 1 {
                    first = index;
                } else if position == 0 {
                    last = index;
                }
                index += 1;
            }

            // These are the actual locals, not serialization copies. In particular,
            // changing length changes the following loop, and first survives a later
            // zero-length block in which no suffix has starting position one.
            io.write((&raw mut length).cast::<u8>(), 4)?;
            io.write((&raw mut first).cast::<u8>(), 4)?;
            io.write((&raw mut last).cast::<u8>(), 4)?;
            index = 0;
            while index < length.wrapping_add(1) {
                io.check_value(
                    Hp2CodecFailure::ArrayIndex,
                    (index as u32) < POSITION_COUNT as u32,
                    index,
                )?;
                let position = positions.add(index as usize).read();
                let source = if position != 0 { position - 1 } else { 0 };
                // Position zero duplicates buffer[0], so output mutation there must
                // be visible if that same byte is emitted again later in this block.
                io.write(buffer.offset(source as isize), 1)?;
                index += 1;
            }
        }
        // Encode never asks for progress, even when a notifier is installed.
        Ok(1)
    }
}

/// # Safety
/// The services and archives must satisfy CodecIo's contract, and their reads
/// and mutations must stay within the original implementation's defined domain
/// described above. The native checks are intentionally not strengthened.
pub unsafe fn decode(io: &CodecIo<'_>) -> CodecResult<i32> {
    // SAFETY: the archive contract establishes initialized reads and valid
    // mutations. Native array checks run before indexed scratch accesses.
    unsafe {
        let mut buffer_storage = scratch::<u8>(POSITION_COUNT);
        let mut temp_storage = scratch::<i32>(POSITION_COUNT);
        let buffer = buffer_storage.as_mut_ptr().cast::<u8>();
        let temp = temp_storage.as_mut_ptr().cast::<i32>();
        let mut length_storage = MaybeUninit::<i32>::uninit();
        let mut first_storage = MaybeUninit::<i32>::uninit();
        let mut last_storage = MaybeUninit::<i32>::uninit();
        let length = length_storage.as_mut_ptr();
        let first = first_storage.as_mut_ptr();
        let last = last_storage.as_mut_ptr();
        let mut totals = [MaybeUninit::<i32>::uninit(); 257];

        while !io.at_end()? {
            io.read(length.cast::<u8>(), 4)?;
            io.read(first.cast::<u8>(), 4)?;
            io.read(last.cast::<u8>(), 4)?;
            io.check(
                Hp2CodecFailure::BwtBlockLimit,
                length.read() <= POSITION_COUNT as i32,
            )?;
            let total_size = io.total_size()?;
            let position = io.tell()?;
            io.check(
                Hp2CodecFailure::BwtRemaining,
                length.read() <= total_size.wrapping_sub(position),
            )?;
            length.write(length.read().wrapping_add(1));
            io.read(buffer, length.read())?;

            let mut counts = [0_i32; 257];
            let mut index = 0_i32;
            while index < length.read() {
                let symbol = if index != last.read() {
                    io.check_value(
                        Hp2CodecFailure::ArrayIndex,
                        (index as u32) < POSITION_COUNT as u32,
                        index,
                    )?;
                    buffer.add(index as usize).read() as usize
                } else {
                    256
                };
                counts[symbol] += 1;
                index += 1;
            }
            let mut sum = 0;
            for symbol in 0..257 {
                totals[symbol].write(sum);
                sum += counts[symbol];
                counts[symbol] = 0;
            }
            index = 0;
            while index < length.read() {
                let symbol = if index != last.read() {
                    io.check_value(
                        Hp2CodecFailure::ArrayIndex,
                        (index as u32) < POSITION_COUNT as u32,
                        index,
                    )?;
                    buffer.add(index as usize).read() as usize
                } else {
                    256
                };
                let target = totals[symbol].assume_init() + counts[symbol];
                counts[symbol] += 1;
                io.check_value(
                    Hp2CodecFailure::ArrayIndex,
                    (target as u32) < POSITION_COUNT as u32,
                    target,
                )?;
                temp.add(target as usize).write(index);
                index += 1;
            }

            index = first.read();
            let mut emitted = 0_i32;
            while emitted < length.read().wrapping_sub(1) {
                io.check_value(
                    Hp2CodecFailure::ArrayIndex,
                    (index as u32) < POSITION_COUNT as u32,
                    index,
                )?;
                io.write(buffer.add(index as usize), 1)?;
                // The update also happens after the final emitted byte, as in the
                // native for-loop, and must retain its native array-bound failure.
                io.check_value(
                    Hp2CodecFailure::ArrayIndex,
                    (index as u32) < POSITION_COUNT as u32,
                    index,
                )?;
                index = temp.add(index as usize).read();
                emitted += 1;
            }

            if io.has_notify() {
                let position = io.tell()? as f32;
                let total_size = io.total_size()? as f32;
                if !io.progress(position / total_size)? {
                    return Ok(0);
                }
            }
        }
        Ok(1)
    }
}
