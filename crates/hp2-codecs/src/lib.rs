//! Port of HP2's small, pure media codecs: EA-XA ADPCM block decode, DXT1
//! block decode, and Vorbis streaming decode. See each module for its
//! exact C++ source and scope.

pub mod dxt1;
pub mod eaxa;
pub mod vorbis;
