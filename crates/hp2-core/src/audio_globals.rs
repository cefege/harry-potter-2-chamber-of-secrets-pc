//! Process-wide audio settings shared by native and canonical Engine callers.
//!
//! The existing C++ ABI reads and writes these symbols directly. Keep one
//! definition so both execution paths observe the same values.

pub static mut GAudioMaxRadiusMultiplier: f32 = 100.0;

pub static mut GAudioDefaultRadius: f32 = 450.0;
