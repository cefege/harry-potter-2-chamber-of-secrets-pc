//! Rust port of the engine's `Render/Src` subsystem.
//!
//! Each C++ file is ported line-by-line; the C++ side keeps only the URender
//! shell and trampolines that forward into the exports in `hp2-ffi`.

pub mod light;
pub mod mesh;
pub mod particle;
pub mod particle_clip;
pub mod particle_shard;
pub mod particle_tube;
pub mod random;
pub mod services;
pub mod services_uren;
pub mod softln;
pub mod span;
pub mod sprite;
pub mod sprite_filter;
pub mod state;
