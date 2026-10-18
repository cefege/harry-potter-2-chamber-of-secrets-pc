# Behavior matrix

This is the ledger of behavioral tests registered in
`Build/CMake/HP2Targets.cmake`. Every row is an `hp2_add_behavior_test`
registration that `ctest` will run; there are no aspirational rows.

> **Superseded parity ledger.** Earlier revisions of this file tracked
> C++-to-Rust parity for a Rust rewrite that has since been removed from this
> tree. Its rows named `hp2rs`, `hp-app`, `hp-engine`, `hp-uobject`,
> `hp-render`, and `hp-audio` surfaces and its gate stages drove
> `Build/parity_gate.py`, which requires a `--rust-bin` that no build in this
> repository produces. Those rows were removed rather than left claiming
> coverage that cannot be earned. `Build/parity_gate.py` itself is still
> present and is listed in `AGENTS.md` under "Known Stale References".

## Data profile registration

Which tests exist at all depends on the configured data root, not on runtime
skipping. `HP2Targets.cmake` sets `HP2_TEST_DATA_PRESENT` from
`${HP2_TEST_DATA_ROOT}/System/Default.ini` and registers data-backed contracts
only when it is present. CMake reports the decision at configure time:

```
-- HP2 test data present at <root>: ON|OFF
```

| Profile | `HP2_TEST_DATA_ROOT` | Registered | CI selection |
| --- | --- | --- | --- |
| `data-none` | a path with no `System/Default.ini` | pure logic, codec, config, launcher, save, and contract tests | `ctest --preset macos-arm64 -L data-none` |
| `data-prototype` | `HarryPotter2/Unreal` (default) | the above plus registration, package79, spell interaction and runtime, input contracts, localization, typography, canvas compatibility, renderer smoke | full `ctest --preset macos-arm64` |
| `data-retail` | an imported retail overlay (`macos-arm64-retail`) | as prototype, against retail assets | full `ctest --preset macos-arm64` |

A test that is not registered is a **configuration gap, not a skip** — it
exists wherever the data does. CI runs the `data-none` column because a fresh
GitHub checkout has no game data (those directories are gitignored). Renderer
smoke additionally requires the packaged bundle, so it is registered only
after `hp2_macos_app` has run.

## How labels are assigned

`hp2_add_behavior_test` labels every test it registers. The data profile is
derived from the arguments: a test that receives `-datadir=` or
`--data-root=` pointed at `HP2_TEST_DATA_ROOT` is labelled `data-prototype`;
everything else defaults to `data-none`. A later
`set_tests_properties(... LABELS ...)` may override that, for example to add
`smoke` or `renderer`.

The default matters. `ctest -L data-none` is the honest run for a data-less
checkout, and a test with no `LABELS` property would be silently omitted from
it — including data-backed tests like `renderer_smoke_xopengl`. Deriving the
label from the arguments means a newly registered data-backed test cannot be
dropped from coverage by forgetting a `set_tests_properties` line elsewhere.
Every registered test carries exactly one data label, so the two selections
partition the suite.

## Registered tests

| Test | Profile | Class | Source |
| --- | --- | --- | --- |
| `abi_widths` | data-none | fast | `Tests/AbiTests.cpp` |
| `audio_lifecycle` | data-prototype | fast | `Tests/AudioTests.cpp` |
| `canvas_compatibility_contracts` | data-prototype | integration | `Tests/CanvasCompatibilityTests.cpp` |
| `command_line_load` | data-none | fast | `Tests/AbiTests.cpp` |
| `compact_index` | data-none | fast | `Tests/AbiTests.cpp` |
| `config_ini_cache_filenames` | data-none | fast | `Tests/ConfigIniTests.cpp` |
| `config_ini_parse_semantics` | data-none | fast | `Tests/ConfigIniTests.cpp` |
| `config_ini_rewrite_normalization` | data-none | fast | `Tests/ConfigIniTests.cpp` |
| `config_ini_set_and_write` | data-none | fast | `Tests/ConfigIniTests.cpp` |
| `config_ini_typed_getters` | data-none | fast | `Tests/ConfigIniTests.cpp` |
| `config_ini_unicode_roundtrip` | data-none | fast | `Tests/ConfigIniTests.cpp` |
| `data_identity_contracts` | data-none | fast | `Tests/DataIdentityTests.cpp` |
| `determinism_double_run` | data-prototype | integration | `Tests/DeterminismDoubleRunTests.py` |
| `dxt1_codec` | data-none | fast | `Tests/Dxt1Tests.cpp` |
| `eaxa_decoder` | data-none | fast | `Tests/EaxaTests.cpp` |
| `framing_baseline_contract` | data-none | fast | `Tests/FramingBaselineTests.py` |
| `fstring_archive` | data-none | fast | `Tests/AbiTests.cpp` |
| `game_test_contract` | data-none | fast | `Tests/GameTestTests.py` |
| `input_axis_order` | data-prototype | integration | `Tests/InputContractTests.cpp` |
| `input_edge_contracts` | data-prototype | integration | `Tests/InputContractTests.cpp` |
| `input_release_all` | data-prototype | integration | `Tests/InputContractTests.cpp` |
| `input_script_contract` | data-none | fast | `Tests/InputScriptSmoke.py` |
| `input_script_smoke` | data-prototype | integration | `Tests/InputScriptSmoke.py` |
| `localization_contract` | data-prototype | integration | `Tests/LocalizationTests.py` |
| `matrix_compare_contract` | data-none | fast | `Tests/MatrixCompareTests.py` |
| `native_launcher_contract` | data-none | fast | `Tests/LauncherTests.cpp` |
| `native_registration` | data-prototype | integration | `Tests/AbiTests.cpp` |
| `native_typography_contracts` | data-prototype | fast | `Tests/NativeTypographyTests.cpp`, `Tests/CanvasCompatibilityTests.cpp` |
| `package79_manifest` | data-prototype | integration | `Tests/PackageAudit.cpp` |
| `projection_fov` | data-none | fast | `Tests/AbiTests.cpp` |
| `prototype_archive_contract` | data-none | fast | `Tests/PrototypeArchiveTests.py` |
| `render_clip` | data-none | fast | `Tests/AbiTests.cpp` |
| `renderer_smoke_xopengl` | data-prototype | integration | `Build/smoke_maps.py` |
| `repair_save_contract` | data-none | fast | `Tests/RepairSaveTests.py` |
| `replay_roundtrip` | data-none | fast | `Tests/ReplayRoundTripTests.cpp` |
| `save_format_contract` | data-none | fast | `Tests/SaveFormatTests.py` |
| `spell_interaction_manifest` | data-prototype | integration | `Build/spell_interaction_audit.py` |
| `spell_runtime_contracts` | data-prototype | integration | `Tests/SpellRuntimeTests.cpp` |
| `ucc_help_smoke` | data-prototype | integration | `Tests/UccSmokeTests.py` |
| `ucc_smoke_contract` | data-none | fast | `Tests/UccSmokeTests.py` |
`Class` is `fast` for a contract that runs in seconds and `integration` for one
that launches the engine or walks a full map.

## Unregistered surfaces

These are work items, not test tiers: generic actor movement/physics,
particles, dynamic movers, frame capture, and retail product flows. A surface
belongs in the table above only once an observable contract and a registered
`hp2_add_behavior_test` exist for it. This prevents an unregistered aspiration
from being mistaken for a passing test.

## Machine-local goldens

`package79_manifest` and `spell_interaction_manifest` compare against
`out/<preset>/goldens/`, which are generated from the configured data root and
are not committed. A build tree that has never run

```sh
cmake --build --preset macos-arm64 --target regenerate_goldens
```

will report those two as failing. That is a setup gap, not a behavior
regression; `Docs/OPERATIONS.md` describes the golden refresh.

See `Docs/ORACLE_BASELINE.md` for the C++ oracle suite baseline and
`Build/game_test.py` for the scenario-report schema.
