//! Borrowed Rust output sink for canonical Engine command dispatch.

use hp2_core::core_access::RuntimeResult;

/// Receives one Engine log message without a C++ FOutputDevice object.
///
/// The sink is borrowed only for the call. It may capture a caller-owned string
/// result or forward messages to Core's configured output function.
pub type ExecOutput<'a> = dyn FnMut(&[u32]) -> RuntimeResult<()> + 'a;

/// Formats a signed INT directly into caller-owned UTF-32 scratch storage.
pub(crate) fn decimal(value: i32, buffer: &mut [u32; 11]) -> &[u32] {
    let mut offset = buffer.len();
    let mut magnitude = value.unsigned_abs();
    loop {
        offset -= 1; buffer[offset] = u32::from(b'0') + magnitude % 10;
        magnitude /= 10;
        if magnitude == 0 { break; }
    }
    if value < 0 { offset -= 1; buffer[offset] = u32::from(b'-'); }
    &buffer[offset..]
}
