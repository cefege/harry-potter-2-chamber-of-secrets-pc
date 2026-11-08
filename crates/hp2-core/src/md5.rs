//! Port of Core's MD5 context and block operations (`UnMisc.cpp`).

use std::ptr::{copy_nonoverlapping, write_bytes};

#[repr(C)]
pub struct Md5Context {
    state: [u32; 4],
    count: [u32; 2],
    buffer: [u8; 64],
}

const SHIFT: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9,
    14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15,
    21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];
const K: [u32; 64] = [
    0xd76a_a478,
    0xe8c7_b756,
    0x2420_70db,
    0xc1bd_ceee,
    0xf57c_0faf,
    0x4787_c62a,
    0xa830_4613,
    0xfd46_9501,
    0x6980_98d8,
    0x8b44_f7af,
    0xffff_5bb1,
    0x895c_d7be,
    0x6b90_1122,
    0xfd98_7193,
    0xa679_438e,
    0x49b4_0821,
    0xf61e_2562,
    0xc040_b340,
    0x265e_5a51,
    0xe9b6_c7aa,
    0xd62f_105d,
    0x0244_1453,
    0xd8a1_e681,
    0xe7d3_fbc8,
    0x21e1_cde6,
    0xc337_07d6,
    0xf4d5_0d87,
    0x455a_14ed,
    0xa9e3_e905,
    0xfcef_a3f8,
    0x676f_02d9,
    0x8d2a_4c8a,
    0xfffa_3942,
    0x8771_f681,
    0x6d9d_6122,
    0xfde5_380c,
    0xa4be_ea44,
    0x4bde_cfa9,
    0xf6bb_4b60,
    0xbebf_bc70,
    0x289b_7ec6,
    0xeaa1_27fa,
    0xd4ef_3085,
    0x0488_1d05,
    0xd9d4_d039,
    0xe6db_99e5,
    0x1fa2_7cf8,
    0xc4ac_5665,
    0xf429_2244,
    0x432a_ff97,
    0xab94_23a7,
    0xfc93_a039,
    0x655b_59c3,
    0x8f0c_cc92,
    0xffef_f47d,
    0x8584_5dd1,
    0x6fa8_7e4f,
    0xfe2c_e6e0,
    0xa301_4314,
    0x4e08_11a1,
    0xf753_7e82,
    0xbd3a_f235,
    0x2ad7_d2bb,
    0xeb86_d391,
];

/// `appMD5Init`.
///
/// # Safety
/// `context` must point to writable `Md5Context` storage.
pub unsafe fn init(context: *mut Md5Context) {
    // SAFETY: required by this function's contract.
    let context = unsafe { &mut *context };
    context.count = [0; 2];
    context.state = [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];
}

/// `appMD5Update`; non-positive lengths perform no reads or state changes.
///
/// # Safety
/// `context` must point to writable `Md5Context` storage. For positive `length`,
/// `input` must be readable for `length` bytes and must not overlap `context`.
pub unsafe fn update(context: *mut Md5Context, input: *const u8, length: i32) {
    if length <= 0 {
        return;
    }
    let length = length as usize;
    // SAFETY: required by this function's contract.
    let context = unsafe { &mut *context };
    // SAFETY: the caller guarantees this range is readable and non-overlapping.
    let input = unsafe { std::slice::from_raw_parts(input, length) };

    let index = ((context.count[0] >> 3) & 0x3f) as usize;
    let bit_increment = (length as u32).wrapping_shl(3);
    let previous = context.count[0];
    context.count[0] = previous.wrapping_add(bit_increment);
    if context.count[0] < bit_increment {
        context.count[1] = context.count[1].wrapping_add(1);
    }
    context.count[1] = context.count[1].wrapping_add((length as u32) >> 29);

    let part_len = 64 - index;
    if length >= part_len {
        // SAFETY: both slices designate valid, disjoint parts of caller/context storage.
        unsafe {
            copy_nonoverlapping(
                input.as_ptr(),
                context.buffer.as_mut_ptr().add(index),
                part_len,
            );
        }
        transform_words(&mut context.state, &context.buffer);
        let mut offset = part_len;
        while offset + 63 < length {
            let block: &[u8; 64] = input[offset..offset + 64]
                .try_into()
                .expect("MD5 block has fixed length");
            transform_words(&mut context.state, block);
            offset += 64;
        }
        context.buffer[..length - offset].copy_from_slice(&input[offset..]);
    } else {
        context.buffer[index..index + length].copy_from_slice(input);
    }
}

/// `appMD5Final`; writes 16 digest bytes and zeroizes the full context.
///
/// # Safety
/// `digest` must point to 16 writable bytes and `context` to writable
/// `Md5Context` storage.
pub unsafe fn final_digest(digest: *mut u8, context: *mut Md5Context) {
    // SAFETY: required by this function's contract.
    let count = unsafe { (*context).count };
    let bits = [
        count[0] as u8,
        (count[0] >> 8) as u8,
        (count[0] >> 16) as u8,
        (count[0] >> 24) as u8,
        count[1] as u8,
        (count[1] >> 8) as u8,
        (count[1] >> 16) as u8,
        (count[1] >> 24) as u8,
    ];
    let index = ((count[0] >> 3) & 0x3f) as usize;
    let pad_len = if index < 56 { 56 - index } else { 120 - index };
    let mut padding = [0u8; 64];
    padding[0] = 0x80;
    // SAFETY: `context` is valid; `padding` and `bits` live through each call.
    unsafe {
        update(context, padding.as_ptr(), pad_len as i32);
        update(context, bits.as_ptr(), bits.len() as i32);
    }
    // SAFETY: context and digest satisfy this function's contract.
    unsafe {
        encode(digest, (*context).state.as_ptr(), 16);
        write_bytes(context.cast::<u8>(), 0, std::mem::size_of::<Md5Context>());
    }
}

/// `appMD5Transform`.
///
/// # Safety
/// `state` must point to four writable words and `block` to 64 readable bytes.
pub unsafe fn transform(state: *mut u32, block: *const u8) {
    // SAFETY: required by this function's contract.
    let state: &mut [u32; 4] = unsafe { &mut *state.cast() };
    // SAFETY: required by this function's contract.
    let block: &[u8; 64] = unsafe { &*block.cast() };
    transform_words(state, block);
}

fn transform_words(state: &mut [u32; 4], block: &[u8; 64]) {
    let mut words = [0u32; 16];
    for (word, bytes) in words.iter_mut().zip(block.chunks_exact(4)) {
        *word = u32::from_le_bytes(bytes.try_into().expect("four-byte word"));
    }

    let [mut a, mut b, mut c, mut d] = *state;
    for step in 0..64 {
        let (function, word_index) = match step {
            0..=15 => ((b & c) | (!b & d), step),
            16..=31 => ((b & d) | (c & !d), (5 * step + 1) % 16),
            32..=47 => (b ^ c ^ d, (3 * step + 5) % 16),
            _ => (c ^ (b | !d), (7 * step) % 16),
        };
        let next = b.wrapping_add(
            a.wrapping_add(function)
                .wrapping_add(K[step])
                .wrapping_add(words[word_index])
                .rotate_left(SHIFT[step]),
        );
        a = d;
        d = c;
        c = b;
        b = next;
    }
    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    words.fill(0);
}

/// `appMD5Encode`; `length` is a multiple of four.
///
/// # Safety
/// `output` must be writable for `length` bytes and `input` readable for
/// `length / 4` words.
pub unsafe fn encode(output: *mut u8, input: *const u32, length: i32) {
    if length <= 0 {
        return;
    }
    let words = (length as usize) / 4;
    // SAFETY: required by this function's contract.
    let input = unsafe { std::slice::from_raw_parts(input, words) };
    // SAFETY: required by this function's contract.
    let output = unsafe { std::slice::from_raw_parts_mut(output, words * 4) };
    for (chunk, word) in output.chunks_exact_mut(4).zip(input) {
        chunk.copy_from_slice(&word.to_le_bytes());
    }
}

/// `appMD5Decode`; `length` is a multiple of four.
///
/// # Safety
/// `output` must be writable for `length / 4` words and `input` readable for
/// `length` bytes.
pub unsafe fn decode(output: *mut u32, input: *const u8, length: i32) {
    if length <= 0 {
        return;
    }
    let words = (length as usize) / 4;
    // SAFETY: required by this function's contract.
    let output = unsafe { std::slice::from_raw_parts_mut(output, words) };
    // SAFETY: required by this function's contract.
    let input = unsafe { std::slice::from_raw_parts(input, words * 4) };
    for (word, chunk) in output.iter_mut().zip(input.chunks_exact(4)) {
        *word = u32::from_le_bytes(chunk.try_into().expect("four-byte word"));
    }
}

#[cfg(test)]
mod tests {
    use super::{Md5Context, decode, encode, final_digest, init, update};

    fn digest(parts: &[&[u8]]) -> [u8; 16] {
        let mut context = Md5Context {
            state: [0; 4],
            count: [0; 2],
            buffer: [0; 64],
        };
        // SAFETY: context and all input slices remain valid for these calls.
        unsafe {
            init(&mut context);
            for part in parts {
                update(&mut context, part.as_ptr(), part.len() as i32);
            }
            let mut result = [0; 16];
            final_digest(result.as_mut_ptr(), &mut context);
            assert_eq!(std::mem::size_of_val(&context), 88);
            assert!(
                std::slice::from_raw_parts((&context as *const Md5Context).cast::<u8>(), 88)
                    .iter()
                    .all(|byte| *byte == 0)
            );
            result
        }
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn rfc1321_vectors_and_chunk_boundaries() {
        let vectors: &[(&[u8], &str)] = &[
            (b"", "d41d8cd98f00b204e9800998ecf8427e"),
            (b"a", "0cc175b9c0f1b6a831c399e269772661"),
            (b"abc", "900150983cd24fb0d6963f7d28e17f72"),
            (b"message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
            (
                b"The quick brown fox jumps over the lazy dog",
                "9e107d9d372bb6826bd81d3542a419d6",
            ),
        ];
        for (input, expected) in vectors {
            assert_eq!(hex(&digest(&[input])), *expected);
            let split = input.len() / 2;
            assert_eq!(hex(&digest(&[&input[..split], &input[split..]])), *expected);
        }
        let long = [b'a'; 1000];
        assert_eq!(
            hex(&digest(&[
                &long[..1],
                &long[1..63],
                &long[63..64],
                &long[64..]
            ])),
            "cabe45dcc9ae5b66ba86600cca6b8ba8"
        );
    }

    #[test]
    fn md5_encode_decode_little_endian_words() {
        let words = [0x0123_4567, 0x89ab_cdef];
        let mut bytes = [0; 8];
        let mut decoded = [0; 2];
        // SAFETY: all arrays provide the declared valid sizes.
        unsafe {
            encode(bytes.as_mut_ptr(), words.as_ptr(), 8);
            decode(decoded.as_mut_ptr(), bytes.as_ptr(), 8);
        }
        assert_eq!(bytes, [0x67, 0x45, 0x23, 0x01, 0xef, 0xcd, 0xab, 0x89]);
        assert_eq!(decoded, words);
    }
}
