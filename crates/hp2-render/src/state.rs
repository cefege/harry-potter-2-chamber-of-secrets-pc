//! Rust-owned URender static state for the `UnRender.cpp` port.
//!
//! Scope: only the statics that no C++ file outside `UnRender.cpp` reaches
//! (grep-verified: `Stamp`, `PointCache`, `GFrameStamp`, and `Dot` have no users
//! outside `UnRender.cpp`). Statics that C++ sprite, mesh, particle, and light
//! code touch (`DynamicsCache`, `SurfLights`, `LeafLights`, `DynLightSurfs`,
//! `DynLightLeaves`, `NumDynLight*`, `Max*Lights`, `VectorMem`) stay
//! C++-owned. Rust reaches them through the `Hp2UrenGlobals` addresses in
//! `services_uren.rs`.
//!
//! One `RenderState` is passed `&mut` into each port function. It is not a
//! global, and it holds no C++ allocation.

use hp2_abi::render_scene::FStampedPoint;

/// `FBspNode::MAX_FINAL_VERTICES` (`Engine/Inc/UnObj.h:100`, value 24).
pub const MAX_FINAL_VERTICES: usize = 24;

/// Rust-owned state from `URender` and file-scope `UnRender.cpp` statics.
#[derive(Debug, Default)]
pub struct RenderState {
    /// `URender::PointCache` (`TArray<URender::FStampedPoint>`, `Render.h:451`).
    /// Element layout: `hp2_abi::render_scene::FStampedPoint`.
    pub point_cache: Vec<FStampedPoint>,
    /// `URender::Stamp` (`DWORD`, `Render.h:453`).
    pub stamp: u32,
    /// `GFrameStamp` (`INT`, `UnRender.cpp:56`).
    pub gframe_stamp: i32,
    /// `Dot[FBspNode::MAX_FINAL_VERTICES]` (`FLOAT`, `UnRender.cpp:1157`).
    pub dot: [f32; MAX_FINAL_VERTICES],
}
