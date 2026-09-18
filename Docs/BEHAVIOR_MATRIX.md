# Behavior matrix

This is the C++-to-Rust parity ledger. It is generated from the C++ behavioral
contracts registered in `Build/CMake/HP2Targets.cmake`, not from a Rust-only
smoke result. A row is complete only when its named evidence artifact exists
for the same qualified data profile and commit.

## Status vocabulary

| Status | Meaning | Gate command and proof artifact |
| --- | --- | --- |
| `covered` | The Rust contract has passed against the corresponding isolated behavioral invariant. It is not a C++/Rust equivalence claim. | `--stage format`, `vm`, or `product`; its `<artifact-dir>/<stage>/stage.json` and named log/diff in the row. |
| `paired` | The C++ oracle and `hp2rs` reached the same manifest locator and every required reported field matched. | `--stage paired` or `world`; `<artifact-dir>/paired/<map>.json` with all checkpoints `match`. |
| `uncovered` | No qualifying artifact proves this C++ contract for Rust yet. Existing source, compilation, and a Rust-only launch do not change this status. | Run the row's named gate stage and retain its named artifact; a prototype artifact does not cover retail. |
| `blocked_data` | Both legs prove the same required asset is absent or corrupt for the same qualified profile. Missing code, an unsupported token, a fallback, or an empty checkpoint is never `blocked_data`. | `--stage paired`; the per-side availability envelopes and paired report must retain the identical asset/profile/proof. |

Current rows are intentionally conservative: the historical G0--G6 migration
table and its Rust-only smoke claims were removed because they are not current
parity evidence. A row promotes only after its stated stage artifact satisfies
every profile named in that row.

## Gate invocation and artifacts

Every qualified run starts with one explicit profile and data root:

```sh
python3 Build/parity_gate.py --profile prototype \
  --data-root HarryPotter2/Unreal \
  --cpp-app dist/macos-arm64/HarryPotter2.app \
  --rust-bin target/release/hp2rs \
  --artifact-dir /tmp/hp2-parity/prototype --stage <format|vm|paired|world|product|all>
```

The gate writes `manifest.json` with the canonical root identity and profile.
Named table artifacts are paths relative to `--artifact-dir`. `product/` is the
workspace and product-contract tranche; `format/`, `vm/`, `paired/`, and
`world/` are data-qualified tranches. A row can cite a Rust source/test surface
without becoming `covered`: the artifact is the proof boundary.

## Registered C++ contracts

The `C++ test` column is the exact `hp2_add_behavior_test` name. Conditional
registrations remain rows because their absence at configure time is a data or
capability configuration fact, not successful Rust parity.

| C++ test | Rust surface | Profile | Status | Required gate and evidence |
| --- | --- | --- | --- | --- |
| `abi_widths` | No Rust ABI-width oracle; integer wire behavior is tracked separately. | none | `uncovered` | `product/cargo-test-workspace.log` plus a Rust ABI contract, when added. |
| `render_clip` | `hp-render/src/math.rs` clip-edge tests. | none | `uncovered` | `product/cargo-test-workspace.log`. |
| `projection_fov` | `hp-render/src/math.rs` projection tests. | none | `uncovered` | `product/cargo-test-workspace.log`. |
| `command_line_load` | `hp-app/src/cli.rs` load-slot parser tests. | none | `uncovered` | `product/cargo-test-workspace.log`. |
| `compact_index` | `hp-format/tests/package79_tests.rs`. | none | `uncovered` | `product/cargo-test-workspace.log`. |
| `fstring_archive` | `hp-format/tests/package79_tests.rs`. | none | `uncovered` | `product/cargo-test-workspace.log`. |
| `native_registration` | `hp-uobject/tests/native_registration.rs`. | prototype | `uncovered` | `vm/native-registration.log`; strict `HP2_PARITY_DATA_ROOT`. |
| `package79_manifest` | `hp-format` `p79audit` example. | prototype, retail | `uncovered` | `format/package79-reference.json`, `format/p79audit.json`, `format/package79-diff.json`. |
| `spell_interaction_manifest` | No Rust interaction manifest equivalent. | prototype | `uncovered` | `vm/script-census.json` and a matched interaction contract. |
| `spell_runtime_contracts` | `hp-uobject/tests/spell_runtime_contracts.rs`. | prototype | `uncovered` | `vm/spell-runtime-contracts.log`; strict `HP2_PARITY_DATA_ROOT`. |
| `dxt1_codec` | `hp-format/tests/trackb_dxt1.rs`. | none | `uncovered` | `product/cargo-test-workspace.log`. |
| `eaxa_decoder` | `hp-format/tests/trackb_eaxa.rs`. | none | `uncovered` | `product/cargo-test-workspace.log`. |
| `replay_roundtrip` | No binary `FReplay::FInputEvent` port yet. | none | `uncovered` | `product/replay-roundtrip.log` after the literal-fixture port. |
| `config_ini_parse_semantics` | `hp-ini/tests/config_ini_oracle.rs`. | none | `uncovered` | `product/cargo-test-workspace.log`. |
| `config_ini_typed_getters` | `hp-ini/tests/config_ini_oracle.rs`. | none | `uncovered` | `product/cargo-test-workspace.log`. |
| `config_ini_set_and_write` | `hp-ini/tests/config_ini_oracle.rs`. | none | `uncovered` | `product/cargo-test-workspace.log`. |
| `config_ini_rewrite_normalization` | `hp-ini/tests/config_ini_oracle.rs`. | none | `uncovered` | `product/cargo-test-workspace.log`. |
| `config_ini_cache_filenames` | `hp-ini/tests/config_ini_oracle.rs`. | none | `uncovered` | `product/cargo-test-workspace.log`. |
| `config_ini_unicode_roundtrip` | `hp-ini/tests/config_ini_oracle.rs`. | none | `uncovered` | `product/cargo-test-workspace.log`. |
| `audio_lifecycle` | `hp-audio/tests/audio_lifecycle.rs`; engine sound ownership remains open. | prototype | `uncovered` | `product/audio-lifecycle.log` and paired active-audio fields. |
| `input_edge_contracts` | `hp-app/src/input.rs` tests. | prototype | `uncovered` | `product/cargo-test-workspace.log`; a paired live input scenario where visible. |
| `input_axis_order` | `hp-app/src/input.rs` tests. | prototype | `uncovered` | `product/cargo-test-workspace.log`; a paired live input scenario where visible. |
| `input_release_all` | `hp-app/src/input.rs` tests. | prototype | `uncovered` | `product/cargo-test-workspace.log`; a paired live input scenario where visible. |
| `mouse_capture_policy` | `hp-app/src/input.rs` policy tests. | prototype | `uncovered` | `product/cargo-test-workspace.log`; a paired live input scenario where visible. |
| `input_event_mapping` | `hp-app/src/input.rs` mapping tests. | prototype | `uncovered` | `product/cargo-test-workspace.log`; a paired live input scenario where visible. |
| `native_launcher_contract` | `hp-app/tests/launcher_oracle.rs`. | none | `uncovered` | `product/launcher-oracle.log`. |
| `game_test_contract` | Shared `Build/game_test.py` comparator only. | none | `uncovered` | `paired/game-test-fixtures.log`. |
| `repair_save_contract` | `hp2rs/tests/repair_save_cli.rs`. | none | `uncovered` | `product/repair-save-cli.log`; real-profile result is also required. |
| `ucc_smoke_contract` | No Rust UCC executable contract. | none | `uncovered` | A Rust commandlet contract or explicit product exclusion. |
| `ucc_help_smoke` | No Rust UCC executable contract. | prototype | `uncovered` | A Rust commandlet contract or explicit product exclusion. |
| `prototype_archive_contract` | `hp-format` archive reader; no equivalent product audit. | prototype | `uncovered` | `format/package79-diff.json` and a matching archive contract. |
| `hp1_loader_manifest` | `hp-format` supports versioned packages; no HP1-qualified audit artifact. | HP1 | `uncovered` | A separately qualified HP1 format artifact; it does not prove HP2 runtime parity. |
| `hp1_class_bind` | No HP1 Rust class-bind runtime contract. | HP1 | `uncovered` | A separately qualified HP1 runtime artifact. |
| `save_format_contract` | Save repair exists; C++ save wire-format fixture has not been ported. | none | `uncovered` | `product/save-format.log`. |
| `localization_contract` | No Rust localization contract. | prototype | `uncovered` | `product/localization.log`. |
| `input_script_contract` | Text `InputScript` is test-driver support, not replay compatibility. | prototype | `uncovered` | `paired/<scenario>.json`. |
| `input_script_smoke` | Rust launch smoke exists but is not a paired interaction proof. | prototype | `uncovered` | `paired/<scenario>.json` and `world/full-map.json`. |
| `framing_baseline_contract` | No C++/Rust checkpoint frame comparison yet. | none | `uncovered` | `paired/<scenario>.json` with observed `frame_hash`. |
| `matrix_compare_contract` | Shared comparison harness; Rust has no independent visual-leg proof. | none | `uncovered` | `paired/game-test-fixtures.log`. |
| `determinism_double_run` | `hp-uobject/tests/retail_determinism.rs` is not engine-wide paired evidence. | prototype | `uncovered` | `paired/<scenario>.json` for deterministic checkpoints. |
| `native_typography_contracts` | Font decoding exists; no native-text compatibility port. | prototype | `uncovered` | `world/canvas-contracts.log`. |
| `canvas_compatibility_contracts` | `Engine.UCanvas` native implementation is absent. | prototype | `uncovered` | `world/canvas-contracts.log` and paired draw telemetry. |
| `renderer_smoke_xopengl` | Rust three-map launch smoke only. | prototype | `uncovered` | `world/trio-map.json` and paired checkpoint reports. |
| `renderer_smoke_full` | Rust 300-tick census only. | prototype, retail | `uncovered` | `world/full-map.json` with `script_deferred=0`. |
| `renderer_smoke_vulkan` | No Rust Vulkan-driver parity contract. | prototype | `uncovered` | A qualified Vulkan paired report with observed `frame_hash`. |

## Unregistered surfaces

These are work items, not test tiers and not parity claims: generic actor
movement/physics, particles, dynamic movers, frame capture, CutScript command
telemetry, and retail product flows. Their evidence is attached to the
registered rows above when an observable C++ contract and an executable gate
exist. This prevents an unregistered aspiration from being mistaken for a
passing C++ contract.

## Evidence promotion

1. `format` byte-compares the canonical Python and Rust package audits for one
   qualified root. Its report may promote only format rows.
2. `vm` records structural and execution ledgers. Any deferral in an executed
   gameplay stream keeps the relevant runtime row `uncovered`.
3. `paired` compares C++ and Rust report locators and every fidelity field. A
   plain launch, an empty report, or `fidelity.checkpoint_not_instrumented`
   cannot promote a row.
4. `world` requires observed matching `frame_hash` values before visual or
   world rows can become `paired`.
5. `product` combines the workspace, replay, launcher/save, packaging, and
   data-profile artifacts. Prototype evidence never promotes a retail row.

See `Docs/ORACLE_BASELINE.md` for the C++ oracle suite baseline and
`Build/game_test.py` for the exact paired-report schema.
