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
| renderer.adapter_unavailable | no Metal/wgpu adapter on this host | hp-render device |
| renderer.device_request_failed | wgpu device request rejected | hp-render device |
| renderer.capture_write_failed | frame PNG/meta write failed | hp-render capture |
| renderer.texture_unknown | texture handle not in manager | hp-render textures |
| renderer.texture_double_destroy | destroy called twice for one handle | hp-render textures |

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

## audio.*

| Code | Meaning | Emitted by |
|---|---|---|
| audio.open_failed | media unreadable, unprobed, or lacking a Vorbis track | hp-audio stream sources |
| audio.decode_failed | decoder hit unrecoverable corruption mid-stream | hp-audio stream sources |
| audio.loop_stalled | looping stream wrapped without producing samples | hp-audio stream sources |
| audio.stream_not_alive | operation on a destroyed or exhausted stream slot | hp-audio StreamManager |
| audio.slot_exhausted | no free stream slot in the manager | hp-audio StreamManager |
| audio.sink_unavailable | output device could not be opened | hp-audio OutputGraph sinks |
| audio.bad_request | invalid argument (empty buffer, zero chunk, wrong kind) | hp-audio stream sources/manager |
| audio.eaxa_feed_failed | EA-XA encoded stream rejected by the block decoder | hp-audio XA source (wraps `eaxa.*`) |

## app.* (zero-argument launch resolution)

| Code | Meaning | Emitted by |
|---|---|---|
| app.datadir_from_store | data root taken from the Launcher.ini Retail assignment | hp2rs zero-arg launch |
| app.picker_shown | no usable stored assignment; the first-run folder picker is displayed | hp2rs zero-arg launch |
| app.datadir_persisted | picker choice validated and committed to the profile store | hp2rs zero-arg launch |
| app.datadir_invalid | chosen or stored folder lacks System/Default.ini | hp2rs zero-arg launch |
| app.datadir_missing | the user cancelled the folder picker; nothing persisted | hp2rs zero-arg launch |
| app.map_autoselected | no map token given; New Game entry map (`PrivetDr.unr`) preferred, else first alphabetical `*.unr` via `[Paths]` | hp2rs zero-arg launch |
| app.map_missing | no `*.unr` maps found under the resolved data root | hp2rs zero-arg launch |
| app.store_unreadable | Launcher.ini present but unparseable | hp2rs zero-arg launch |
| app.store_unwritable | launcher root cannot be created / store commit failed | hp2rs zero-arg launch |

## engine.* (Phase 3 headless simulation)

| Code | Meaning | Emitted by |
|---|---|---|
| engine.map_unreadable | map file missing or unreadable at the resolved token path | hp-engine level bootstrap |
| engine.map_parse | package79 parse rejected the map archive | hp-engine level bootstrap |
| engine.map_name_index | name-table index out of range while loading a map | hp-engine level bootstrap |
| engine.map_export_gap | map exports failed to instantiate (summary after per-export notes) | hp-engine level bootstrap |
| engine.actor_tag_range | actor property tag name index out of range; payload kept raw | hp-engine actor decode note |
| engine.actor_tag_size | negative tagged-property size in an actor payload | hp-engine actor decode note |
| engine.actor_tag_raw | tag kept raw: template missing or width mismatch (note-class, non-fatal) | hp-engine actor decode notes |
| engine.script_event_deferred | actor BeginPlay/Tick skipped under the accepted Phase-3 deferral policy (see Docs/RNG_TICK_DIVERGENCE.md) | hp-engine tick orchestration |
| engine.arg_ticks / arg_seed / arg_fixed_dt / arg_map_twice / arg_unknown / arg_load | CLI contract rejections with usage text | hp2rs main |
| engine.ini_missing / ini_parse | config layer absent or unparseable at bootstrap | hp2rs main |
| engine.input_script_* | recorded-script grammar violations (`_field`, `_tick_twice`, `_float`, `_count`, `_key`, `_op`, `_no_tick`, `_unreadable`) | hp-engine input parser |
| engine.io | unexpected filesystem failure outside map/config paths | hp-engine |

## vm.* additions (hp-uobject, Phase 3)

| Code | Meaning | Emitted by |
|---|---|---|
| vm.unknown_token_deferrable | unknown opcode inside a fork-undocumented gap (0x03/0x35/0x5B-0x6F); loud, and deferral-eligible for script events | hp-uobject VM |
| vm.switch_case_expected | EX_Switch case chain walked off a non-EX_Case token | hp-uobject VM |
| native.slot_unbound / native.body_deferred | numbered-native dispatch found no registered body / a registered-but-deferred body (existing codes, now reachable from script events) | hp-uobject natives registry |

## blocked reasons (status=blocked)

`binary_missing`, `data.prototype_missing`, `data.archive_missing`,
`data.bag_incomplete`, `renderer.icd_missing`, `renderer.binary_missing`,
`mechanism.unavailable`, `bundle.unsigned` (warning-class), and
`engine_does_not_emit` (budget telemetry absent).
