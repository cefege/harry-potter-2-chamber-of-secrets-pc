# HP2 reason-code catalog

Single authority for every dotted lowercase `reason_code` emitted by
reports, bootstrap rejections, and crash records. When adding a code,
add it here in the same commit. Codes are `<domain>.<specific>`; a
missing code is represented as JSON `null`.

## data.*

| Code | Meaning | Emitted by |
|---|---|---|
| data.manifest_missing | overlay-manifest.json absent on a manifest-bearing root | HP2Paths bootstrap |
| data.checksums_missing | overlay-manifest.json present without overlay-checksums.txt | HP2Paths bootstrap |
| data.manifest_invalid | manifest unparseable/schema violation | HP2Paths bootstrap |
| data.path_set_mismatch | manifest and checksums disagree on the file set | HP2Paths bootstrap |
| data.size_mismatch | size disagreement between indexes or file | HP2Paths bootstrap |
| data.hash_mismatch | sha256 disagreement or corrupted bytes | HP2Paths bootstrap |
| data.profile_unknown | manifest declares an unknown profile | HP2Paths bootstrap |
| data.prototype_missing | prototype tree absent for a data-prototype test | Localization/UCC wrappers |
| data.archive_missing | prototype-data.7z not present | verify_prototype_archive |
| data.bag_incomplete | bag manifests do not cover the payload | verify_prototype_archive |
| data.identity_gap | archive member set vs bag mismatch | verify_prototype_archive |
| data.archive_listing_failed | bsdtar listing failed/corrupt archive | verify_prototype_archive |
| data.mutated | data-root inventory changed across a run | UCC smoke wrapper |

## renderer.*

| Code | Meaning | Emitted by |
|---|---|---|
| renderer.capability_compile_time_disabled | native text built out | text backend status |
| renderer.fonts_unavailable | CoreText registry unusable | text backend status |
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
| save.header_bad_tag | save magic/tag invalid | repair_save (proposed) |
| save.engine_load_failed | native loader rejected synthetic golden | SaveFormatTests |
| settings.<transform> | migration journal entries (boolean_spelling, screen_mode_consolidated, viewport_consolidated, range_*, enum_*, ...) | launcher store migrations |
| crash.signal | fatal signal delivered | HP2CrashReporter |
| crash.watchdog.timeout | watchdog deadline exceeded | HP2CrashReporter |

## blocked reasons (status=blocked)

`binary_missing`, `data.prototype_missing`, `data.archive_missing`,
`data.bag_incomplete`, `renderer.icd_missing`, `renderer.binary_missing`,
`mechanism.unavailable`, `bundle.unsigned` (warning-class), and
`engine_does_not_emit` (budget telemetry absent).
