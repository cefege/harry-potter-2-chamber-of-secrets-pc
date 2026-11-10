//! Boot order, command-line, portable configuration, main-loop policy and runtime
//! reporters used by `hp2-game`, plus the settings `model` and `store` that remain the
//! parity reference for the stock in-game menus. The startup launcher UI and its
//! round, profile, import and save-selection policy were retired.

pub mod boot;
pub mod boot_config;
pub mod command_policy;
pub mod main_loop;
pub mod model;
pub mod reporters;
pub mod store;
