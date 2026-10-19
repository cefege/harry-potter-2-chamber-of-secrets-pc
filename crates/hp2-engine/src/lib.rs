//! Engine-side ports (`Engine/Src/UnReach.cpp`, `UnWind.cpp`). C++ fills
//! `services::Hp2EngineServices` once per call site and passes a pointer on
//! every export, the same way the Render port does. `appRand`/`appFrand`,
//! level and model queries stay in C++ behind that table.

pub mod actcol;
pub mod cam_mgr;
pub mod canvas;
pub mod canvas_text;
pub mod console;
pub mod coords;
pub mod dynbsp;
pub mod fpoly;
pub mod gesture;
pub mod input;
pub mod levact;
pub mod lodmesh;
pub mod math;
pub mod mesh;
pub mod model;
pub mod mover;
pub mod palette;
pub mod params;
pub mod particle;
pub mod pawn;
pub mod pawn_native_ai;
pub mod pawn_native_files;
pub mod pawn_native_input;
pub mod pawn_native_move;
pub mod pawn_reach;
pub mod pawn_sense;
pub mod physic;
pub mod player;
pub mod prim;
pub mod reach;
pub mod route;
pub mod scr_tex;
pub mod services;
pub mod skeletal;
pub mod stat_log;
pub mod tex;
pub mod trace;
pub mod uengine;
pub mod url;
pub mod vec;
pub mod wind;
