//! Port of the non-Apple branch of `HP2PortableSha256.h`'s self-contained
//! FIPS 180-4 SHA-256 implementation (`CC_SHA256_CTX` / `CC_SHA256_*`).

use std::ptr::copy_nonoverlapping;

#[repr(C)]
pub struct Sha256Context {
    state: [u32; 8],
    bit_count: u64,
    buffer: [u8; 64],
    buffer_length: usize,
}

const INITIAL_STATE: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

const ROUND_CONSTANTS: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

/// `CC_SHA256_Init`.
///
/// # Safety
/// `context` must point to writable `Sha256Context` storage.
pub unsafe fn init(context: *mut Sha256Context) {
    // SAFETY: required by this function's contract.
    let context = unsafe { &mut *context };
    context.state = INITIAL_STATE;
    context.bit_count = 0;
    context.buffer_length = 0;
}

/// `CC_SHA256_Update`.
///
/// # Safety
/// `context` must point to writable `Sha256Context` storage. For positive
/// `length`, `data` must be readable for `length` bytes and must not overlap
/// `context`.
pub unsafe fn update(context: *mut Sha256Context, data: *const u8, length: u32) {
    // SAFETY: required by this function's contract.
    let context = unsafe { &mut *context };
    context.bit_count = context.bit_count.wrapping_add((length as u64) * 8);

    let mut remaining = length as usize;
    let mut bytes = data;
    while remaining > 0 {
        let space = 64 - context.buffer_length;
        let take = if space < remaining { space } else { remaining };
        // SAFETY: the caller guarantees `data` is readable for `length` bytes
        // starting at `data`; `bytes` only advances within that range, and
        // `context.buffer` has room for `take` bytes starting at `buffer_length`.
        unsafe {
            copy_nonoverlapping(bytes, context.buffer.as_mut_ptr().add(context.buffer_length), take);
        }
        context.buffer_length += take;
        // SAFETY: `bytes` stays within the caller-guaranteed readable range.
        bytes = unsafe { bytes.add(take) };
        remaining -= take;

        if context.buffer_length == 64 {
            transform(&mut context.state, &context.buffer);
            context.buffer_length = 0;
        }
    }
}

/// `CC_SHA256_Final`; writes 32 digest bytes. Matches `CC_SHA256_Final`,
/// which does not zeroize the context afterward.
///
/// # Safety
/// `digest` must point to 32 writable bytes and `context` to writable
/// `Sha256Context` storage.
pub unsafe fn final_digest(digest: *mut u8, context: *mut Sha256Context) {
    // SAFETY: required by this function's contract.
    let bit_count = unsafe { (*context).bit_count };

    let pad_high: u8 = 0x80;
    // SAFETY: `context` satisfies this function's contract; `pad_high` lives
    // through the call.
    unsafe { update(context, &pad_high, 1) };
    let pad_low: u8 = 0x00;
    // SAFETY: `context` satisfies this function's contract.
    while unsafe { (*context).buffer_length } != 56 {
        // SAFETY: `context` satisfies this function's contract; `pad_low`
        // lives through the call.
        unsafe { update(context, &pad_low, 1) };
    }

    let mut length_bytes = [0u8; 8];
    for (index, byte) in length_bytes.iter_mut().enumerate() {
        *byte = (bit_count >> (56 - index * 8)) as u8;
    }
    // SAFETY: `context` satisfies this function's contract; `length_bytes`
    // lives through the call.
    unsafe { update(context, length_bytes.as_ptr(), 8) };

    // SAFETY: `digest` is writable for 32 bytes per this function's contract.
    let digest = unsafe { std::slice::from_raw_parts_mut(digest, 32) };
    // SAFETY: `context` is valid per this function's contract.
    let state = unsafe { (*context).state };
    for (word, chunk) in state.iter().zip(digest.chunks_exact_mut(4)) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
}

fn transform(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut schedule = [0u32; 64];
    for (word, bytes) in schedule[..16].iter_mut().zip(block.chunks_exact(4)) {
        *word = u32::from_be_bytes(bytes.try_into().expect("four-byte word"));
    }
    for index in 16..64 {
        let s0 = schedule[index - 15].rotate_right(7)
            ^ schedule[index - 15].rotate_right(18)
            ^ (schedule[index - 15] >> 3);
        let s1 = schedule[index - 2].rotate_right(17)
            ^ schedule[index - 2].rotate_right(19)
            ^ (schedule[index - 2] >> 10);
        schedule[index] = schedule[index - 16]
            .wrapping_add(s0)
            .wrapping_add(schedule[index - 7])
            .wrapping_add(s1);
    }

    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;

    for index in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let choose = (e & f) ^ (!e & g);
        let temp1 = h
            .wrapping_add(s1)
            .wrapping_add(choose)
            .wrapping_add(ROUND_CONSTANTS[index])
            .wrapping_add(schedule[index]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let majority = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = s0.wrapping_add(majority);

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp1);
        d = c;
        c = b;
        b = a;
        a = temp1.wrapping_add(temp2);
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

#[cfg(test)]
mod tests {
    use super::{Sha256Context, final_digest, init, update};

    fn digest(parts: &[&[u8]]) -> [u8; 32] {
        let mut context = Sha256Context {
            state: [0; 8],
            bit_count: 0,
            buffer: [0; 64],
            buffer_length: 0,
        };
        // SAFETY: context and all input slices remain valid for these calls.
        unsafe {
            init(&mut context);
            for part in parts {
                update(&mut context, part.as_ptr(), part.len() as u32);
            }
            let mut result = [0; 32];
            final_digest(result.as_mut_ptr(), &mut context);
            result
        }
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn fips_180_4_vectors_and_chunk_boundaries() {
        // NIST FIPS 180-4 / well-known SHA-256 test vectors.
        let vectors: &[(&[u8], &str)] = &[
            (
                b"",
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                b"abc",
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            (
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
                "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
            ),
        ];
        for (input, expected) in vectors {
            assert_eq!(hex(&digest(&[input])), *expected);
            let split = input.len() / 2;
            assert_eq!(hex(&digest(&[&input[..split], &input[split..]])), *expected);
        }

        // One million repeated 'a' characters: another FIPS 180-4 vector,
        // exercised in chunks to cross multiple 64-byte blocks.
        let million_a = vec![b'a'; 1_000_000];
        assert_eq!(
            hex(&digest(&[
                &million_a[..1],
                &million_a[1..63],
                &million_a[63..64],
                &million_a[64..]
            ])),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn context_layout_matches_cc_sha256_ctx() {
        assert_eq!(std::mem::size_of::<Sha256Context>(), 112);
        assert_eq!(std::mem::offset_of!(Sha256Context, state), 0);
        assert_eq!(std::mem::offset_of!(Sha256Context, bit_count), 32);
        assert_eq!(std::mem::offset_of!(Sha256Context, buffer), 40);
        assert_eq!(std::mem::offset_of!(Sha256Context, buffer_length), 104);
    }
}
