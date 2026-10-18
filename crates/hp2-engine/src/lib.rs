//! Engine-side ports (`Engine/Src/UnReach.cpp`, `UnWind.cpp`). C++ fills
//! `services::Hp2EngineServices` once per call site and passes a pointer on
//! every export, the same way the Render port does. `appRand`/`appFrand`,
//! level and model queries stay in C++ behind that table.

pub mod math;
pub mod reach;
pub mod services;
pub mod vec;
pub mod wind;
