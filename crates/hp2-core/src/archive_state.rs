//! FArchive constructor scalar state over the compiler-probed native layout.
//! The caller owns the vptr; padding is never read or written here, including
//! tail padding reused by derived classes such as ULinkerLoad.

use hp2_abi::engine::generated::{
    ARCHIVE_ENGINE_NEGOTIATION_VERSION, CORE_LINKER_CONSTANTS, FArchive,
};
use std::ptr;

/// Initialize only the three version fields and eight archive status fields.
///
/// # Safety
/// `archive` must address the FArchive subobject's writable, compiler-probed
/// scalar fields for the duration of this call, with exclusive access to those
/// fields. They need not be initialized. No whole-object reference is formed:
/// the native vptr, opaque bytes, and reused tail padding remain untouched.
pub const unsafe fn initialize_scalar_state(archive: *mut FArchive) {
    unsafe {
        ptr::addr_of_mut!((*archive).ar_ver).write_unaligned(CORE_LINKER_CONSTANTS.version);
        ptr::addr_of_mut!((*archive).ar_net_ver)
            .write_unaligned(ARCHIVE_ENGINE_NEGOTIATION_VERSION);
        ptr::addr_of_mut!((*archive).ar_licensee_ver)
            .write_unaligned(CORE_LINKER_CONSTANTS.licensee);
        ptr::addr_of_mut!((*archive).ar_is_loading).write_unaligned(0);
        ptr::addr_of_mut!((*archive).ar_is_saving).write_unaligned(0);
        ptr::addr_of_mut!((*archive).ar_is_trans).write_unaligned(0);
        ptr::addr_of_mut!((*archive).ar_is_persistent).write_unaligned(0);
        ptr::addr_of_mut!((*archive).ar_for_edit).write_unaligned(1);
        ptr::addr_of_mut!((*archive).ar_for_client).write_unaligned(1);
        ptr::addr_of_mut!((*archive).ar_for_server).write_unaligned(1);
        ptr::addr_of_mut!((*archive).ar_is_error).write_unaligned(0);
    }
}
