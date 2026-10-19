# C++ Oracle Baseline — Acceptance Record

Date: 2026-08-24. Tag: `cpp-oracle-baseline` → commit `53c6078`
("option-a: real allocator at main entry; universal package path resolution",
branch `option-a-publishable`).

## Command

```
git tag cpp-oracle-baseline
ctest --preset macos-arm64 --output-on-failure
```

## Observed result

**39/41 passed, 2 failed** (~45 s total):

| Test | Result | Evidence |
|---|---|---|
| `determinism_double_run` | Failed (blocked) | `run_1` launch exit_status=1, no failure markers |
| `renderer_smoke_xopengl` | Failed | 0/3 maps passed, `process.exit_status` |

All other 39 tests passed unchanged.

## Failure signature

Both failures share one signature against the freshly-built
`out/macos-arm64/hp2_game`: the process dies during
`UGameEngine::Init -> LoadMap -> VerifyPackages ->
StaticLoadObject(Engine.Level, ..\Maps\<Map>.unr)` with
`General protection fault!` (handled by HP2_CRASH, exit 1). Reproduced
manually via `Build/game_test.py run` with the fresh binary on the prototype
data root (`Maps/PrivetDr.unr`, 30 ticks): exit 1, zero failure markers,
same History line.

Attribution facts (diagnostic session, 2026-08-24):

- Stale installed bundle `dist/macos-arm64/HarryPotter2.app` (built
  2026-08-23 ~20:04, predating the last three option-a commits) passes the
  same launch on BOTH prototype and retail roots (exit 0, zero markers).
- Fresh build at `53c6078^` segfaults earlier still
  (`FString::operator+=` → `wcscpy(NULL)` in `main`) — the launch-blocking
  allocator-ordering bug that `53c6078` itself fixes; that commit's message
  documents it.
- The regression window is therefore the recent option-a series
  (`5bef6c8..53c6078`); the exact faulting commit was NOT isolated.
  Debugging was stopped by explicit user decision: the C++ tree is slated
  for deletion at cutover, and the stale working bundle suffices as the
  comparison oracle.

## Disposition (user-approved)

- Do NOT repair the C++ regression. Do NOT rebuild C++ engine binaries
  during Phases 0–5.
- **Frozen behavioral oracle = `dist/macos-arm64/HarryPotter2.app`**
  (last-known-good binary). Known limitation: its frame-capture hook may be
  older than the current one; revisit only if Phase 4 baseline generation
  requires newer capture behavior.
- G0's "C++ suite re-run green untouched" is satisfied as
  *re-run-identical*: after the harness-only `--engine-bin` threading
  change, the suite must reproduce exactly this 39/41 with the same two
  failures. Any delta beyond these two tests implicates the harness change.
- These two tests remain excluded from parity claims; all Rust-gate
  comparisons use the game_test/smoke protocols directly against the frozen
  oracle binary, not the C++ determinism/renderer tests.
- Fresh-built non-engine tools are not needed for G1 format parity: the
  audit diff is Python `Build/package79_reference.py` vs Rust `p79audit`;
  no C++ binary participates.

## Unresolved conditions

- Faulting commit inside the option-a window unidentified (deliberately).
  Revisit only if the Rust port must distinguish old-vs-new loader
  behavior; it should mirror the working bundle's behavior.
- Retail-profile claims above verified by manual launch reproduction only
  (30 ticks, PrivetDr), not the full retail preset suite.

## Addendum 2026-08-24 — baseline upgraded to 41/41

The user fixed the loader regression in commit `353ed22` ("option-a:
launcher context globals, driver presentation, robust teardown"). After
rebuilding (`cmake --build --preset macos-arm64`) and re-running
`ctest --preset macos-arm64`: **100% tests passed out of 41**, including the
previously red `determinism_double_run` and `renderer_smoke_xopengl`.

Consequences:

- The C++ oracle is fully green again; future gate comparisons expect 41/41.
- `determinism_double_run` / `renderer_smoke_xopengl` re-enter the parity
  set; the "excluded tests" caveat above is retired.
- Phase 4 may use freshly built binaries (current capture hooks) for render
  baselines instead of the stale bundle.

## In-place port: fixture stub acceptance (2026-10-19)

The current in-place port pins each fixture generator to its own pre-port
source ref; the historical whole-game baseline above is not a substitute
for those objects.

`Build/fixture_common.py` now reserves zero-filled, 16-byte-aligned 1 MiB BSS
slots for unresolved data symbols. The old 64-byte slots were smaller than
macOS arm64's 192-byte `jmp_buf` used by `__Context::Env`; `setjmp` could
overwrite neighbouring stub storage. Function stubs still abort when reached.
The reservation is not a general proof of arbitrary external symbol sizes.

Acceptance command: `python3 /tmp/hp2_fixture_stub_regen.py` in `/tmp/m2wt`.
That throwaway driver ran every tracked `Build/*_fixture_harness/gen_*.sh`
with the absolute `out/macos-arm64` directory, retained each generator's log,
and compared SHA-256 hashes of all tracked Rust fixture files after each run.

Observed: **29/29 generators exited 0; all 94 tracked fixture files remained
byte-identical.** No fixture refresh or golden acceptance is implied.
Evidence:
`/tmp/m2wt/out/macos-arm64/Testing/HP2/fixture_stub_regen/Artifacts/`
(`baseline.json`, `results.json`, and per-generator logs).

Profile: **data-none**, using the generators' synthetic inputs and pinned C++
objects on macOS arm64. This does not establish retail, Linux, Windows, or
Raspberry Pi runtime parity.

## In-place `ULevel::PostLoad` iterator port (2026-10-19)

The actor-XLevel retarget loop in `ULevel::PostLoad` now runs in Rust. The
service table exposes only opaque `TObjectIterator<AActor>` lifecycle,
dereference, outer lookup, and XLevel assignment leaves. Exhausted iterators
are checked before dereference; constructor exceptions return through the
pending-exception bridge and are rethrown by the C++ shell.

Acceptance command: `sh /tmp/gates.sh level Build/level_fixture_harness/gen_level.sh level_oracle.txt ULevel`
in `/tmp/m2wt`.

Observed on macOS arm64: build passed; data-none CTest **30/30** and runtime
CTest **2/2** passed; workspace tests **725 passed, 0 failed**; workspace and
renderer FFI clippy passed with `-D warnings`; `cargo fmt --check` passed;
ABI check passed; `level_oracle.txt` regenerated byte-identically (**1127
lines**); the `level_core_replay` suite passed **16/16**. A/B Wgpu smoke
passed **8/8** maps under **data-prototype** (`Entry`, `PrivetDr`,
`Ch2Skurge`, `TrenchRun`, `Dada`, `Adv1Willow`, `Duel10`, `Adv6Goyle`).

Unverified: data-retail and Linux. Logs and reports were preserved in
`out/macos-arm64/Testing/HP2/ULevelPostLoad/Artifacts/`, including the CTest,
build, ABI, Clippy, workspace-test, fixture-regeneration, and A/B smoke output.
