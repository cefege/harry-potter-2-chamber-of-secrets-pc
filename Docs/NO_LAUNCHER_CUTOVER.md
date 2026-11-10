# No-launcher cutover — product direction and bounded plan

**Status: startup window retired and verified at test level (macOS arm64);
functional migration gaps remain.** See "Cutover and retirement record" below.

## Cutover and retirement record

- `boot::run_main` no longer asks for or runs a launcher round. Missing data or an
  invalid `-datadir` exits 1 before any window; bootstrap's diagnostic now points to
  `hp2_import` / `-datadir`. No-arg boot with discoverable data goes straight to
  `Startup.unr` (log evidence in `boot-cutover/ancestor-data-discovery/`). Removed:
  `launcher_ui.rs`, `launcher_window.rs`, the launcher round in `boot_services.rs`,
  `should_run_native_launcher`, four boot leaves, `launcher_ui_contracts`,
  `Tests/LauncherSmoke.py`, and hp2-game's egui/egui-wgpu/wgpu/hp2-import/hp2-ini
  dependencies (35 lockfile packages). Source-shape gates were updated for the two
  removed `BootHost` fields and uses.
- Accepted behavior changes: saved `Launcher.ini` data-source/profile selection and
  launcher New Game/Continue prefix are no longer applied at startup; the default
  Core INI and global user path are both the base User directory, so the earlier
  profile/INI mismatch no longer occurs in the default flow. No in-game data import
  exists: first run needs `hp2_import` (documented, not a UI).
- 67% render scale: the stock slider now uses 12 ordinal positions (0..11), so 67%
  is reachable; persisted/live values stay percentages. The retail replay fixture
  gains an eleventh click with idle compensation (574 frames preserved). This is
  unrun: the retail replay needs retail data.
- `hp2-core`: `native_runtime_init::initialize` is now `unsafe`; new actual-owner
  tests `core_owner_paths_contracts` (success and five-Paths failure) show the
  published global user path selects Save while Core INIs stay at the explicit files.
- Evidence: `retirement/suite/summary.txt` (25 ctest checks plus three native-free
  closure checks passed after the final build), `boot-cutover/`, `scale-integration/`.
  Still unverified: physical stock-menu use, retail data, native/oracle parity,
  Linux, and in-game data import.
- Later cleanup: 13 unreachable `hp2-launcher` modules (round, profile/migration,
  save discovery, import session, presentation policy, atomic/tree, display modes,
  feedback, capabilities, selection policy; originals in
  `retirement/launcher-policy-before/`) and its `hp2-import` dependency were removed,
  keeping boot, boot_config, command_policy, main_loop, reporters, and `model`/`store`
  (the settings parity reference). The retired launcher render-scale label/fallback
  tests are gone with their code. hp2-game no longer enables `hp2-platform`'s `nfd`
  feature or links `nfd::nfd`; the picker library and its 28-case tests remain.

## Current bounded evidence

Initial boot, settings and config-owner maps are at `Artifacts/no-launcher-cutover/`
under the BeautifulRust artifact root. Main independently reviewed the relevant
source and records its limited reading coverage in `main-source-review.md`.

- `runtime-configuration-check-oyybvn1a/`: exact `paths_bootstrap_contracts` passes
  all 16 Rust cases, including one parent that checks 7 fresh child-process cases.
  Ten malformed/escaping synthetic fixture inputs fail closed as expected. Both
  exact agent gates pass (21 checker fixtures), and the explicit configured-list
  check finds all 8 required CI names/labels. Main accepted retained source, file,
  producer and executable evidence in `main-evidence-inspection.json`.
- The new test child shares the existing HOME/cwd-test lock. It requires a private,
  fresh, authorized fixture before writes. The earlier unsafe fixture proposal was
  corrected before any compiler/test execution. This is not protection against a
  hostile concurrent same-user writer. Content/layout checks do not cover all
  filesystem metadata. The production prefix, shipping products and pins stay
  unchanged; the profile installer in this test only records arguments.
- `scale-policy-candidate-utf1eifb/`: original copied transaction tests pass 6; the
  new ordinal-spec test fails before the change (retained exit 101); the isolated
  candidate passes 9. This is not an integrated control, loaded-package VM, GPU or
  physical menu result. The existing 10-click retail replay would need 11 clicks
  for the proposed 12-position control; no fixture/golden was refreshed.

- `slider-package-passive-z2dkn0ab/`: Main reviewed the complete inventory script
  and the used parser/resolver/terminal routines, then ran the passive Python3.13
  inventory once (exit0). Four checked-in prototype package snapshots/hash checks
  and 3 complete serialized UFunction spans match copied `.bin`/`.hex` outputs.
  The expected class chain and unambiguous terminal candidates are recorded. This
  decodes no script bounds, operands, jumps or reachable calls; VM invocation is
  still unapproved. There are no Rust features or SDK calls in this inventory.

Other evidence here is macOS arm64/default bootstrap features/synthetic data-none
unless explicitly described otherwise. Configuration characterization does not
prove Core global/save ownership or fix the profile/INI mismatch. The expression
decoder design and VM test source remain unexecuted proposals needing review.

## Accepted product direction

The user explicitly clarified that there must be **no external/startup launcher
UI**. Launcher functionality belongs in the game's settings/options UI, including
its graphical settings. This supersedes the proposed hardening or unsafe-API
change for the existing `launcher_ui::run_window` entrypoint.

Do not replace the launcher with another external window, rename it as an options
window, silently bypass it while dropping its functions, or restore the deleted
in-game overlay. Existing stock in-game menu integration is substantial concurrent
work and must be preserved. Inspect its actual owners, callers and evidence before
editing it. The presence of a menu module does not prove a working setting.

The internal `hp2-launcher` package currently also holds bootstrap, command-line,
configuration, save and reporting policy. Its name is not a reason to delete those
functions or break the game/UCC/import products. Separate UI removal, functional
migration, unused-code/dependency cleanup and documentation changes.

## Current production path, not the target

- `crates/hp2-launcher/src/boot.rs::run_main` conditionally calls `RunLauncherRound`.
- `crates/hp2-game/src/boot_services.rs` implements that round and calls
  `hp2_game::launcher_ui::run_window`.
- `crates/hp2-game/src/launcher_ui.rs` supplies settings/import/selection UI state;
  `launcher_window.rs` owns a separate SDL/Metal/WGPU window.
- `crates/hp2-engine/src/stock_*` and settings modules already contain in-game
  menu/provider integration. Their modified/untracked files and existing overlay
  deletions predate this new bounded task; no ownership is assumed here.

The current tree therefore does not yet meet the clarified product direction.

## Functional inventory required before removal

For each existing launcher control or action, record its in-game entry, actual
runtime owner, persistent storage, restart/live-apply semantics, error/rollback
behavior and behavioral/process test. Include:

- Display mode/resolution/fullscreen, timeout/revert and restart behavior.
- All 12 render scales, including 67%, without changing existing fallbacks.
- Renderer/quality/texture/object-detail, FPS and text settings, preserving
  `[Display] NativeText=False` bitmap compatibility and feature-gate states.
- Input, audio and any other existing settings.
- New-game/continue/save choices and command-line overrides.
- Data-root/source selection, import, validation/provenance and persistence.

First-run missing-data handling is an explicit design/verification gap. The game
cannot be assumed to load a stock menu without its required packages. A missing
root must not reopen the removed launcher, silently choose another profile, be
reported as retail qualification, or lose its recoverable diagnostic. Keep the
standalone importer CLI available; whether it suffices for initial provisioning
must be decided against the requested in-game functionality, not silently assumed.

## Stages and acceptance

1. **Inventory/review:** read the current boot, configuration and menu owners;
   inspect relevant diffs; map covered functions and real gaps. Retain evidence at
   `out/macos-arm64/Testing/HP2/beautiful_rust/Artifacts/no-launcher-cutover/`.
2. **Functional migration:** add missing in-game entry/provider/persistence paths
   with separate baseline/candidate evidence. Do not introduce a second overlay or
   change defaults, bytes, callbacks or SDK ownership as incidental cleanup.
3. **Startup cutover:** start the game without the external window. Preserve CLI,
   selected/explicit data-root handling, configuration, save semantics, errors and
   ordered resource teardown. Do not hard-code a `false` policy and call all
   functionality migrated.
4. **Retirement:** remove the external window and its unreachable UI wiring after
   call-path proof. Keep reusable settings/import/bootstrap policy. Review obsolete
   tests and dependencies explicitly; do not delete them merely to obtain green.
5. **Qualification:** preserve all three complete producer trees and prior logs
   before execution. Inspect each exact required CTest, run with
   `--no-tests=error`, verify nonempty counts, and retain first failures. Qualify
   default launch, explicit map/CLI, configuration persistence, in-game settings
   entry/application/restart/revert, and missing-data behavior at the appropriate
   profile. Use only the explicit verification build target until packaging/oracle
   prerequisites are resolved. No release/retail/physical UI claim from unit tests.

Main owns shared source coordination, CMake/docs and all execution. The initial
workers have read-only/artifact-only boot and settings-map tasks. No source edit
lease or compiler/product execution is granted by this plan.

## Compiled-slider decode and loaded-script test (final)

- `decode_package_functions.py` (retained under `A/no-launcher-cutover/`; source-derived
  from `expression_archive.rs::Walk`) decoded 14 compiled slider functions from the
  checked-in prototype `UWindow.u`/`HGame.u` (`slider-decode-uwindow`,
  `slider-decode-hgame`): CheckValue, SetValue, KeyDown, MouseMove, LMouseDown/Up,
  Paint, BeforePaint, SetRange, GetValue, Created and the three HPMenuOptionHSlider
  overrides. Every function's logical script size and serialized trailer matched
  exactly, with valid jump boundaries. The bytecode round to `Step`, clamp to
  `MinValue`/`MaxValue`, move by `Step` on a track click, and contain no 50/100
  constants, so the 0..11 step-1 range is generic script behavior. Notify only calls
  `NotifyWindow` when it is non-None. This is prototype data only, not retail.
- `stock_render_scale_slider_vm` (data-prototype, new, ignored-by-default Rust test
  `stock_render_scale_control::tests::loaded_slider_checkvalue_setvalue_and_scale_mapping`)
  runs the real loaded `HPMenuOptionHSlider` script: old 50..100/step 5 baseline,
  0..11/step 1 rounding and clamping, and every valid `Scale` surviving the script's
  rounding back through `Scale::from_slider`. It passed 1/1 (`slider-vm/`).
- Data import decision: an in-game import cannot exist for the missing-data first run,
  because every stock menu page is loaded from the game's own packages. Adding an
  import page would mean editing game data and is out of scope. `hp2_import` plus the
  game's exit-1 guidance is the supported path; this is a decision, not a gap.
- Still requiring a person or other assets: physical stock-menu use, retail data and
  native-oracle runs. Linux was excluded.

## Retail verification (macOS arm64, local retail overlay)

Retail data found at `~/Library/Application Support/Harry Potter 2/Data/Retail`
(455 MB, `overlay-manifest.json` + 488-entry `overlay-checksums.txt`). A disposable
copy plus the overlay's own checksum list (self-consistency only, not original-media
authenticity) was used; the personal install was only read and still verifies. Evidence:
`A/retail-scale/`.

- **Map sweep:** `Build/smoke_maps.py`, all 42 retail maps, 300 ticks, wgpu, the final
  game binary `e7abb566…`: 42/42 launched, exit 0, no failure markers, timeouts, leaks
  or orphaned processes (24 min). Windowed launch, synthetic data; not audio, not
  human play, not a native-oracle comparison.
- **Stock render-scale replay:** with 11 clicks the slider walked
  100, 95, 90, 85, 80, 75, 70, 67, 65, 60, 55, 50; 0.5 persisted; the restart phase
  showed 50% and the GPU requested 50% with scaled world scopes; the retail copy was
  unchanged. The harness still reports "engine errors observed" because the retail
  `PrivetIntro` cutscene logs four `CutError` lines in both phases; that criterion was
  not weakened and is not evidence about the slider. PNG checkpoints still need a
  human look.
