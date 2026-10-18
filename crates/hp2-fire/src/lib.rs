//! Rust port of `HarryPotter2/Unreal/Fire/Src/UnFractal.cpp`.
//!
//! The UObject class shells, `IMPLEMENT_CLASS`, native registration and member
//! layout stay in C++. Algorithm bodies live here and are called through the
//! `hp2_rs_fire_*` exports in `crates/hp2-ffi`.
//!
//! Modules:
//! * `abi`: shared `#[repr(C)]` types and trace hook table.
//! * `globals`: process-global RNG and phase tables.
//! * `tables`: `InitTables` port.
//! * `fire`: fire diffusion (`CalculateFire` and its wrap and slow variants).
//! * `palette`: Blue Laguna palette.
//! * `sparks`: spark editor, spark redraw, flash ramp.
//! * `water`: water simulation, drops, water paint.
//! * `wet`: wet-texture bump mapping.
//! * `ice`: ice texture blit and panning.
//! * `wave`: wave bump mapping.

pub mod abi;
pub mod fire;
pub mod globals;
pub mod ice;
pub mod palette;
pub mod sparks;
pub mod tables;
pub mod water;
pub mod wave;
pub mod wet;
