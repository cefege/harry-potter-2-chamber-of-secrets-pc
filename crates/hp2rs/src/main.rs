//! `hp2rs` — the engine binary. Phase 0 scaffold: the executable exists so
//! the harness `--engine-bin` contract has a target to grow into; Phase 3
//! wires the real CLI and headless smoke mode here.

fn main() {
    // Scaffold placeholder: succeed without touching GPU or game data.
    // Replaced by hp-app's event loop ownership in Phase 3/5.
    let _ = hp_format::CRATE_NAME;
}
