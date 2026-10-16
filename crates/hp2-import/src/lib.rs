//! Rust port of `HarryPotter2/Unreal/Import/Src/HP2Import*.cpp` (the
//! `prepare_retail_data.py`-equivalent importer that builds the runtime's
//! `HarryPotter2/Unreal` overlay). Ported incrementally, one piece at a
//! time; see the plan for the full scope.

pub mod classify;
pub mod files;
pub mod hash;
pub mod lower;
