# HP2 reason-code catalog

Single authority for every dotted lowercase `reason_code` emitted by
reports, bootstrap rejections, and crash records. When adding a code,
add it here in the same commit. Codes are `<domain>.<specific>`; a
missing code is represented as JSON `null`.

Scope: codes emitted by the current C++ tree. Entries that described the
removed Rust rewrite (`hp2rs`, `hp-app`, `hp-engine`, `hp-uobject`,
`hp-render`, `hp-audio`) were removed rather than left as aspirational
rows - nothing in this repository emits them. The orphaned scripts that
referenced that tree are still present and are called out in
`AGENTS.md` under "Known Stale References".

## data.identity_* (data-root bootstrap rejection)

Emitted by the native bootstrap in
`HarryPotter2/Unreal/Launch/Src/HP2Paths.cpp` as a stderr record
`HP2_DATA_IDENTITY_REJECT <code> <detail>`, and surfaced through
`HP2PathsBootstrapStatus::ReasonCode` / `ValidateHP2DataRootStatus`.
Covered by `data_identity_contracts` (see `Tests/DataIdentityTests.cpp`).

| Code | Meaning | Emitted by |
|---|---|---|
| data.manifest_missing | `overlay-checksums.txt` present without `overlay-manifest.json` | HP2Paths VerifyOverlayIdentity |
| data.checksums_missing | `overlay-manifest.json` without a sibling `overlay-checksums.txt` | HP2Paths VerifyOverlayIdentity |
| data.manifest_invalid | manifest is unparseable, or has an unsupported `schema_version` / non-string `profile` / no `entries` / malformed entry | HP2Paths VerifyOverlayIdentity |
| data.profile_unknown | manifest `profile` is not `safe`, `full`, or `retail-only` | HP2Paths VerifyOverlayIdentity |
| data.path_set_mismatch | manifest and checksums index describe different path sets, or a path is duplicated or unsafe | HP2Paths VerifyOverlayIdentity |
| data.size_mismatch | an entry's recorded size differs from the file on disk | HP2Paths VerifyOverlayIdentity |
| data.hash_mismatch | an entry's SHA-256 is malformed or differs from the streamed file digest | HP2Paths VerifyOverlayIdentity |

## data.*

| Code | Meaning | Emitted by |
| data.prototype_missing | prototype tree absent for a data-prototype test | Localization/UCC wrappers |
| data.archive_missing | prototype-data.7z not present | verify_prototype_archive |
| data.bag_incomplete | bag manifests do not cover the payload | verify_prototype_archive |
| data.identity_gap | archive member set vs bag mismatch | verify_prototype_archive |
| data.archive_listing_failed | bsdtar listing failed/corrupt archive | verify_prototype_archive |
| data.mutated | data-root inventory changed across a run | UCC smoke wrapper |

## renderer.*

| Code | Meaning | Emitted by |
|---|---|---|
| renderer.binary_missing | gate-on hp2_game/icd not present | run_vulkan_smoke |
| renderer.icd_missing | MoltenVK ICD json missing | run_vulkan_smoke |
| engine.vulkan_device_absent | launch log lacks Vulkan device line | run_vulkan_smoke |

## process.*

| Code | Meaning | Emitted by |
|---|---|---|
| process.launch_error | executable failed to start | smoke_maps/game_test |
| process.exit_status | nonzero engine exit | smoke_maps/game_test |
| process.timeout | wall-clock budget/timeout hit | smoke_maps/run_vulkan_smoke |
| process.orphaned_group | process group survived cleanup | smoke_maps |
| process.cleanup_error | cleanup itself failed | smoke_maps |

## content and budgets

| Code | Meaning | Emitted by |
|---|---|---|
| package.structure | structural parse failure before markers | smoke_maps |
| marker.<category> | captured log matched a failure-marker category | smoke_maps/game_test |
| resources.leak | created vs destroyed resource counts disagree | smoke_maps |
| budget.map_seconds_exceeded | per-map wall budget exceeded | smoke_maps |
| budget.frame_ms_exceeded | frame-time budget exceeded | smoke_maps (when engine emits timing) |

## feature/save/settings/crash

| Code | Meaning | Emitted by |
|---|---|---|
| capability.compile_time_disabled | feature compiled out | hp2-state / backend status |
| settings.<transform> | migration journal entries (boolean_spelling, screen_mode_consolidated, viewport_consolidated, range_*, enum_*, ...) | launcher store migrations |
| crash.signal | fatal signal delivered | hp2-platform `crash_reporter` |
| crash.watchdog.timeout | watchdog deadline exceeded | hp2-platform `crash_reporter` |

## blocked reasons (status=blocked)

`binary_missing`, `data.prototype_missing`, `data.archive_missing`,
`data.bag_incomplete`, `renderer.icd_missing`, `renderer.binary_missing`,
`mechanism.unavailable`, `bundle.unsigned` (warning-class), and
`engine_does_not_emit` (budget telemetry absent).
