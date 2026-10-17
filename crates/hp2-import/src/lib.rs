//! Rust port of `HarryPotter2/Unreal/Import/Src/HP2Import*.cpp` (the
//! `prepare_retail_data.py`-equivalent importer that builds the runtime's
//! `HarryPotter2/Unreal` overlay). Ported incrementally, one piece at a
//! time; see the plan for the full scope.
//!
//! Two pieces of this crate deviate from the plan's literal text
//! (archive extraction's crate choice; the casefold table's data
//! source) -- both user-approved, dated, and reasoned in the plan
//! document's "Approved deviations" section, not silent drift. See
//! [`lower`] and [`fold_table`] for the casefolding one specifically.

pub mod classify;
pub mod collect;
pub mod files;
mod fold_table;
pub mod hash;
pub mod lower;
pub mod plan;
mod util;
