//! `#[repr(C)]` mirrors of the transformed-vertex types in
//! `Engine/Inc/UnRender.h` (`FTransform`, `FTransTexture`), used by the
//! particle port (`hp2-render::particle`). Layout is pinned by the
//! `layout FTransTexture` line of `tests/fixtures/particle_reference.txt`
//! (captured from the original C++) and by the asserts below.

use crate::{FPlane, FVector};

/// `FTransform : FOutVector { FVector Point; BYTE Flags; }` plus
/// `FLOAT ScreenX, ScreenY; INT IntY; FLOAT RZ;`. 32 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FTransform {
    pub point: FVector,
    pub flags: u8,
    pub screen_x: f32,
    pub screen_y: f32,
    pub int_y: i32,
    pub rz: f32,
}

const _: () = assert!(size_of::<FTransform>() == 32);
const _: () = assert!(std::mem::offset_of!(FTransform, point) == 0);
const _: () = assert!(std::mem::offset_of!(FTransform, flags) == 12);
const _: () = assert!(std::mem::offset_of!(FTransform, screen_x) == 16);
const _: () = assert!(std::mem::offset_of!(FTransform, screen_y) == 20);
const _: () = assert!(std::mem::offset_of!(FTransform, int_y) == 24);
const _: () = assert!(std::mem::offset_of!(FTransform, rz) == 28);

/// `FTransTexture : FTransSample : FTransform { FPlane Normal, Light, Fog; }`
/// plus `FLOAT U, V;`. 88 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FTransTexture {
    pub transform: FTransform,
    pub normal: FPlane,
    pub light: FPlane,
    pub fog: FPlane,
    pub u: f32,
    pub v: f32,
}

const _: () = assert!(size_of::<FTransTexture>() == 88);
const _: () = assert!(std::mem::offset_of!(FTransTexture, transform) == 0);
const _: () = assert!(std::mem::offset_of!(FTransTexture, normal) == 32);
const _: () = assert!(std::mem::offset_of!(FTransTexture, light) == 48);
const _: () = assert!(std::mem::offset_of!(FTransTexture, fog) == 64);
const _: () = assert!(std::mem::offset_of!(FTransTexture, u) == 80);
const _: () = assert!(std::mem::offset_of!(FTransTexture, v) == 84);
