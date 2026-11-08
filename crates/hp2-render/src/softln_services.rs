//! Checked live SDK access for software projection and debug geometry.
//! Native owners and typed lifetimes stay in the caller; no owner borrow is
//! retained across a leaf, including recursive drawing and error reporting.
//!
//! `RendMap`/`Zoom`/`Fx2`/`Fy2`/`ProjZ`/`Coords`/`IsOrtho`/`OrthoZoom`/
//! `UnitCoords`/`TrigTable`/`Location` are all mirror reads (`FSceneNode`,
//! `UViewport`, `APlayerPawn`) or Rust-owned globals, so they never cross the
//! callback boundary. Only `RenderLine`/`DeviceLine` reach native methods
//! (`URender::Draw3DLine`/`URenderDevice::Draw3DLine`) and stay leaves.

use core::ffi::c_void;
use hp2_abi::engine::generated::{APlayerPawn, UViewport};
use hp2_abi::render_scene::FSceneNode;
use hp2_abi::{FCoords, FVector};

/// `ERendMap` values (`Engine/Inc/UnCamera.h`) spanning `UViewport::IsOrtho`.
const REN_ORTH_XY: i32 = 13;
const REN_ORTH_YZ: i32 = 15;

/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2SoftlnLeaf {
    RenderLine,
    DeviceLine,
}

#[repr(C)]
pub struct Hp2SoftlnLine {
    pub points: [f32; 6],
    pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2SoftlnServices {
    pub context: *mut c_void,
    pub invoke:
        Option<unsafe extern "C" fn(*mut c_void, Hp2SoftlnLeaf, *const c_void, *mut c_void) -> i32>,
}

/// Copied service identity and borrowed frame; never a borrow of a live
/// native engine owner across a leaf.
#[derive(Clone, Copy)]
pub struct Native {
    services: Hp2SoftlnServices,
    frame: *const FSceneNode,
}

impl Native {
    /// # Safety
    /// The service record, callback context, and `frame` must remain valid
    /// until return.
    pub unsafe fn from_raw(
        services: *const Hp2SoftlnServices,
        frame: *const c_void,
    ) -> Result<Self, ()> {
        if services.is_null() || frame.is_null() {
            return Err(());
        }
        let services = unsafe { services.read() };
        if services.invoke.is_none() {
            return Err(());
        }
        Ok(Self {
            services,
            frame: frame.cast(),
        })
    }

    /// `Frame->Viewport->Actor`, or `None` if either pointer is null.
    fn actor(&self) -> Option<*const APlayerPawn> {
        unsafe {
            let viewport = (*self.frame).viewport;
            if viewport.is_null() {
                return None;
            }
            let actor = (*viewport.cast::<UViewport>()).actor;
            if actor.is_null() {
                return None;
            }
            Some(actor.cast())
        }
    }

    pub fn rend_map(&self) -> Result<i32, ()> {
        let actor = self.actor().ok_or(())?;
        Ok(unsafe { (*actor).rend_map })
    }
    pub fn zoom(&self) -> Result<f32, ()> {
        Ok(unsafe { (*self.frame).zoom })
    }
    pub fn fx2(&self) -> Result<f32, ()> {
        Ok(unsafe { (*self.frame).fx2 })
    }
    pub fn fy2(&self) -> Result<f32, ()> {
        Ok(unsafe { (*self.frame).fy2 })
    }
    pub fn proj_z(&self) -> Result<f32, ()> {
        Ok(unsafe { (*self.frame).proj.z })
    }
    pub fn coords(&self) -> Result<FCoords, ()> {
        Ok(unsafe { (*self.frame).coords })
    }
    /// `UViewport::IsOrtho`: `Actor->RendMap` is one of the three ortho maps.
    pub fn is_ortho(&self) -> Result<bool, ()> {
        let rend_map = self.rend_map()?;
        Ok((REN_ORTH_XY..=REN_ORTH_YZ).contains(&rend_map))
    }
    pub fn ortho_zoom(&self) -> Result<f32, ()> {
        let actor = self.actor().ok_or(())?;
        Ok(unsafe { (*actor).base.ortho_zoom })
    }
    /// Rust-owned global; never crosses the native boundary.
    pub fn unit_coords(&self) -> Result<FCoords, ()> {
        Ok(hp2_core::math::unit_coords())
    }
    /// Rust-owned global; never crosses the native boundary.
    pub fn trig_table(&self) -> Result<&'static [f32; hp2_core::math::NUM_ANGLES], ()> {
        Ok(hp2_core::math::trig_table())
    }

    fn line(
        &self,
        leaf: Hp2SoftlnLeaf,
        start: FVector,
        end: FVector,
        flags: u32,
    ) -> Result<(), ()> {
        let line = Hp2SoftlnLine {
            points: [start.x, start.y, start.z, end.x, end.y, end.z],
            flags,
        };
        let status = unsafe {
            (self.services.invoke.unwrap())(
                self.services.context,
                leaf,
                (&line as *const Hp2SoftlnLine).cast(),
                core::ptr::null_mut(),
            )
        };
        if status == 0 { Ok(()) } else { Err(()) }
    }
    pub fn render_line(&self, start: FVector, end: FVector, flags: u32) -> Result<(), ()> {
        self.line(Hp2SoftlnLeaf::RenderLine, start, end, flags)
    }
    pub fn device_line(&self, start: FVector, end: FVector, flags: u32) -> Result<(), ()> {
        self.line(Hp2SoftlnLeaf::DeviceLine, start, end, flags)
    }
}
