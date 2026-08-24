//! Byte-exact UE1 package (versions 60..=79) reading, writing, tagged
//! properties, and data-root audit parity.
//!
//! Normative behavior source: `Build/package79_reference.py`; property-grammar
//! cross-reference `Build/smoke_maps.py` `_tagged_properties`.

mod archive;
mod audit;
pub mod cursor;
pub mod error;
mod json;
pub mod properties;
pub mod summary;
pub mod tables;

pub use archive::{PackageArchive, read_package, round_trip, write_package};
pub use audit::{
    AuditedPackage, FUNCTION_FLAG_MASK, FUNCTION_FLAG_NATIVE, FUNCTION_FLAG_NET,
    FUNCTION_FLAG_NET_RELIABLE, FunctionTerminal, MAX_NATIVE_INDEX, ObjectPaths,
    assemble_audit_document, audit_entry, derive_function_terminal,
};
pub use cursor::{
    ByteCursor, LICENSEE_VERSION, MAX_NAME_UNITS, PACKAGE_MAX_VERSION, PACKAGE_MIN_VERSION,
    PACKAGE_TAG, encode_compact_index, latin1_str, read_compact_index, read_fstring,
    write_compact_index, write_fstring,
};
pub use error::{PackageError, PackageResult};
pub use json::Json;
pub use properties::{
    PROPERTY_TYPE_BOOL, PROPERTY_TYPE_STR, PROPERTY_TYPE_STRUCT, PropertyTag, read_property_tags,
    write_property_tag, write_property_terminator,
};
pub use summary::{GenerationEntry, Guid, PackageSummary, TableDirectory};
pub use tables::{ExportEntry, ImportEntry, NameEncoding, NameEntry};
