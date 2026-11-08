//! SDK diagnostic reporters ported from `SDLLaunch.cpp`'s JSON scenario
//! parsers and observer-driven trace/report writers. Each submodule owns one
//! reporter's state, parsing and output byte-for-byte; `SDLLaunch.cpp` keeps
//! one thin adapter class per C++ observer interface, forwarding virtuals to
//! this crate's `extern "C"` exports (see `crates/hp2-ffi`).

pub mod json;
pub mod actor_slot_dump;
pub mod actor_transition_ledger;
pub mod creature_generator;
pub mod fidelity;
pub mod fire_texture;
pub mod global_tick;
pub mod lifecycle_gate;
pub mod shadow_admission;
pub mod static_bsp_probe;
pub mod trace_services;
pub mod world_collision_gate;
