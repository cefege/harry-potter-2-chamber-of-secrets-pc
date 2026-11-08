//! Legacy RLE and move-to-front download formats.
use crate::download_codec::{CodecIo, CodecResult, Hp2CodecFailure};

unsafe fn emit_run(io: &CodecIo<'_>, mut byte: u8, mut count: u8) -> CodecResult<()> {
    for _ in 0..count.min(5) {
        // Native output callbacks can mutate this same local between writes.
        unsafe {
            io.write(&raw mut byte, 1)?;
        }
    }
    if count >= 5 {
        unsafe {
            io.write(&raw mut count, 1)?;
        }
    }
    Ok(())
}

/// # Safety
/// The checked service table and synchronous native archive owners are live.
pub unsafe fn rle_encode(io: &CodecIo<'_>) -> CodecResult<i32> {
    let mut previous = 0u8;
    let mut count = 0u8;
    let mut byte = 0u8;
    while !io.at_end()? {
        unsafe {
            io.read(&raw mut byte, 1)?;
        }
        if byte != previous || count == 255 {
            unsafe {
                emit_run(io, previous, count)?;
            }
            previous = byte;
            count = 0;
        }
        count = count.wrapping_add(1);
    }
    unsafe {
        emit_run(io, previous, count)?;
    }
    Ok(1)
}

/// # Safety
/// As rle_encode. Original uninitialized reads are outside the native contract.
pub unsafe fn rle_decode(io: &CodecIo<'_>) -> CodecResult<i32> {
    let mut count = 0i32;
    let mut previous = 0u8;
    let mut byte = 0u8;
    let mut run = 0u8;
    while !io.at_end()? {
        unsafe {
            io.read(&raw mut byte, 1)?;
            io.write(&raw mut byte, 1)?;
        }
        if byte != previous {
            previous = byte;
            count = 1;
        } else {
            count = count.wrapping_add(1);
            if count == 5 {
                unsafe {
                    io.read(&raw mut run, 1)?;
                }
                io.check(Hp2CodecFailure::RleRunCount, run >= 2)?;
                loop {
                    let old = run;
                    run = run.wrapping_sub(1);
                    if old <= 5 {
                        break;
                    }
                    unsafe {
                        io.write(&raw mut byte, 1)?;
                    }
                }
                count = 0;
            }
        }
        if io.has_notify() {
            let position = io.tell()? as f32;
            let size = io.total_size()? as f32;
            if !io.progress(position / size)? {
                return Ok(0);
            }
        }
    }
    Ok(1)
}

/// # Safety
/// The checked service table and synchronous native archive owners are live.
pub unsafe fn mtf_encode(io: &CodecIo<'_>) -> CodecResult<i32> {
    let mut list: [u8; 256] = std::array::from_fn(|i| i as u8);
    let mut byte = 0u8;
    let mut encoded: u8;
    while !io.at_end()? {
        unsafe {
            io.read(&raw mut byte, 1)?;
        }
        let index = list.iter().position(|&b| b == byte).unwrap_or(256);
        io.check(Hp2CodecFailure::MtfIndex, index < 256)?;
        encoded = index as u8;
        unsafe {
            io.write(&raw mut encoded, 1)?;
        }
        // Use byte after output: native callbacks can retain its input address.
        for i in (1..=index).rev() {
            list[i] = list[i - 1];
        }
        list[0] = byte;
    }
    Ok(1)
}

/// # Safety
/// As mtf_encode; input and output locals remain mutable across native calls.
pub unsafe fn mtf_decode(io: &CodecIo<'_>) -> CodecResult<i32> {
    let mut list: [u8; 256] = std::array::from_fn(|i| i as u8);
    let mut encoded = 0u8;
    let mut byte: u8;
    while !io.at_end()? {
        unsafe {
            io.read(&raw mut encoded, 1)?;
        }
        byte = list[encoded as usize];
        unsafe {
            io.write(&raw mut byte, 1)?;
        }
        // Both encoded and byte can have changed during native output.
        for i in (1..=encoded as usize).rev() {
            list[i] = list[i - 1];
        }
        list[0] = byte;
        if io.has_notify() {
            let position = io.tell()? as f32;
            let size = io.total_size()? as f32;
            if !io.progress(position / size)? {
                return Ok(0);
            }
        }
    }
    Ok(1)
}
