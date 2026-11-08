//! Rust port of the engine's `Render/Src` subsystem.
//!
//! Each C++ file is ported line-by-line; the C++ side keeps only the URender
//! shell and trampolines that forward into the exports in `hp2-ffi`.

pub mod actor_sprite;
pub mod console;
#[cfg(feature = "coretext")]
pub mod coretext_sys;
pub mod dynamic_items;
#[cfg(feature = "freetype")]
pub mod fontconfig_sys;
#[cfg(feature = "freetype")]
pub mod freetype_sys;
#[cfg(feature = "freetype")]
pub mod harfbuzz_sys;
pub mod lifecycle;
pub mod light;
pub mod light_actor;
pub mod light_info;
pub mod light_manager;
pub mod light_surf;
pub mod mesh;
pub mod mesh_lod_pipeline;
pub mod mesh_pipeline;
pub mod mesh_shared;
pub mod native_text_layout;
pub mod native_text_layout_coretext;
#[cfg(feature = "coretext")]
pub mod native_text_layout_coretext_host;
pub mod native_text_layout_freetype;
#[cfg(feature = "freetype")]
pub mod native_text_layout_freetype_host;
pub mod particle;
pub mod particle_clip;
pub mod particle_shard;
pub mod particle_system;
pub mod particle_tube;
pub mod post_render;
pub mod post_timing;
pub mod pre_render;
pub mod precache;
pub mod random;
pub mod scene_pipeline;
pub mod services;
pub mod services_light;
pub mod services_uren;
pub mod softln;
pub mod softln_draw;
pub mod softln_services;
pub mod span;
pub mod sprite;
pub mod sprite_filter;
pub mod state;
pub mod stats;
pub mod stats_presentation;
