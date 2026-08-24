//! Byte-exact UE1 package (versions 60..=79) reading, writing, tagged
//! properties, and data-root audit parity.
//!
//! Normative behavior source: `Build/package79_reference.py`; property-grammar
//! cross-reference `Build/smoke_maps.py` `_tagged_properties`.

mod cursor;
mod error;

pub use cursor::{
    ByteCursor, LICENSEE_VERSION, MAX_NAME_UNITS, PACKAGE_MAX_VERSION, PACKAGE_MIN_VERSION,
    PACKAGE_TAG, encode_compact_index, latin1_str, read_compact_index, read_fstring,
    write_compact_index, write_fstring,
};
pub use error::{PackageError, PackageResult};
