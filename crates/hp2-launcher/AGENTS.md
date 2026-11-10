# Launcher policy and support

Read the root [handbook](../../AGENTS.md), [architecture](../../Docs/ARCHITECTURE.md),
[Rust constraints](../../Docs/RUST_STYLE.md), and
[worker contract](../../Docs/WORKER_CONTRACTS.md). Keep tasks bounded to a policy,
its real host, and the resource owner. This crate is an unpublished application
library, not the shipping process or a native SDK owner.

## Product direction

The user requires no external/startup launcher UI. `boot::run_main` no longer runs a
launcher round and `hp2-game` no longer contains the egui host. Settings belong in
the stock in-game menus. This crate's remaining launcher-only policy (profiles,
`Launcher.ini` sources, saves discovery, import sessions, presentation policy) is
unreachable from boot; retire or migrate it only after proving each caller. Follow
[the cutover plan](../../Docs/NO_LAUNCHER_CUTOVER.md). Do not restore a startup window
or deleted overlays.

## Current production boundaries

- `boot.rs` controls boot order. `crates/hp2-game/src/main.rs` drives its phases;
  `crates/hp2-game/src/boot_services.rs` owns `BootHost`, the Engine owner, and teardown.
- `main_loop.rs` supplies loop policy. Trace `crates/hp2-game/src/main_loop_services.rs`
  before changing callback or borrowing behavior.
- The launcher round, profile/migration, save discovery, import session, presentation
  policy and `launcher_ui.rs` host were retired; recover them from git history only
  if a reviewed in-game replacement needs a specific behavior.
- `model.rs` and `store.rs` define settings and persistence. Inspect their current
  diffs before editing: settings work can be concurrent.
- `reporters/` is policy and state, not proof of safe engine-object access. Trace
  `crates/hp2-game/src/reporters.rs`, `crates/hp2-game/src/gate_reporters.rs`, and the Engine observer before
  changing pointer reads, callback attribution, reset, or detach order.

Several source comments describe removed C++ ownership or exception scopes.
Treat them as historical reference, not evidence of a current native owner.
Preserve the actual Rust/SDK call path and report conflicting descriptions.

## Focused checks and invariants

- For settings changes, inspect and run exact `engine_settings_launcher_parity` and
  `no_startup_launcher_contracts` CTests with `--no-tests=error`.
- For render-scale changes, run `stock_render_scale_policy`, `stock_render_scale_control`,
  `stock_render_scale_launcher_parity` and `stock_render_scale_replay_contracts`. The
  stock slider uses 12 ordinal positions; the retired launcher's label/fallback tests
  no longer exist, and no physical window or retail run backs these results.
- Preserve 67%, changed-only persistence and the stock fallback behavior; check the
  complete slider position mapping, not one selected value.
- Reporter and filesystem changes require their own characterization and caller
  evidence. Compilation, a pure policy test, or a trace marker is not proof of
  reentry, teardown, ABI, or native parity.

Use [the execution ledger](../../Docs/BEAUTIFUL_RUST_STATUS.md) for current audit
coverage and unresolved conditions. Full reading, caller review, and runtime
qualification are separate states; do not mark one complete from another.
