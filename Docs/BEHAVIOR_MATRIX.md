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
| `core_parse_integer` | data-none | fast | `Tests/AbiTests.cpp` |
| `core_parse_scalar` | data-none | fast | `Tests/AbiTests.cpp` |
| `core_parse_guid` | data-none | fast | `Tests/AbiTests.cpp` |
| `core_guid_generation` | data-none | fast | `Tests/AbiTests.cpp` |
| `core_native_path` | data-none | fast | `Tests/AbiTests.cpp` |
| `core_parse_object` | data-none | fast | `Tests/AbiTests.cpp` |
| `core_pure_ansi` | data-none | fast | `Tests/AbiTests.cpp` |
| `core_trim_spaces` | data-none | fast | `Tests/AbiTests.cpp` |
| `core_memswap` | data-none | fast | `Tests/AbiTests.cpp` |
| `core_ceil_log_two` | data-none | fast | `Tests/AbiTests.cpp` |
| `core_crc` | data-none | fast | `Tests/AbiTests.cpp` |
| `core_spc` | data-none | fast | `Tests/AbiTests.cpp` |
| `core_md5` | data-none | fast | `Tests/AbiTests.cpp` |
| `core_app_format` | data-prototype | integration | `Tests/AbiTests.cpp` |
| `core_app_fext` | data-none | fast | `Tests/AbiTests.cpp` |
| `core_app_path_leaf` | data-none | fast | `Tests/AbiTests.cpp` |
| `config_ini_cache_filenames` | data-none | fast | `Tests/ConfigIniTests.cpp` |
| `config_ini_parse_semantics` | data-none | fast | `Tests/ConfigIniTests.cpp` |
| `config_ini_rewrite_normalization` | data-none | fast | `Tests/ConfigIniTests.cpp` |
| `config_ini_set_and_write` | data-none | fast | `Tests/ConfigIniTests.cpp` |
| `config_ini_typed_getters` | data-none | fast | `Tests/ConfigIniTests.cpp` |
| `config_ini_unicode_roundtrip` | data-none | fast | `Tests/ConfigIniTests.cpp` |
| `data_identity_contracts` | data-none | fast | `Tests/DataIdentityTests.cpp` |
| `paths_bootstrap_contracts` | data-none | fast | `crates/hp2-launcher/src/bootstrap_paths.rs` — native manifest byte/number/duplicate-key handling, streamed package identity and rejection precedence, and real-filesystem weak-canonical directory terminators after unresolved components |
| `determinism_double_run` | data-prototype | integration | `Tests/DeterminismDoubleRunTests.py` |
| `actor_is_animating` | data-none | fast | `crates/hp2-engine/src/actor.rs` |
| `actor_unactor_contracts` | data-none | fast | `crates/hp2-engine/src/actor.rs`, `crates/hp2-engine/src/actor_anim.rs`, `crates/hp2-engine/tests/actor_unactor.rs` — includes full `PlayAnim` state/tween/replication transitions, Move root handling, auxiliary-channel cleanup/dispatch, failure logging and callback status propagation |
| `actor_process_state_contracts` | data-none | fast | `crates/hp2-engine/src/lev_tic.rs`, `crates/hp2-engine/tests/process_state_contracts.rs` — Rust-owned ProcessState skip precedence/outcome tracing, authority-or-simulated dispatch, latent gating, VM loop and fifth-transition cap; observer on every normal path (including skips), ordered service-failure propagation without later observer calls. C++ retains state access, Step/GNatives, guard/timer/random-trace scopes and lifecycle/trace emission in `UnScript.cpp`. |
| `actor_remote_function_contracts` | data-none | fast | `crates/hp2-engine/src/actor_rpc.rs`, `crates/hp2-engine/tests/actor_rpc_contracts.rs` — Rust-owned ProcessRemoteFunction return/Absorb and demo-before-network ordering, authority ownership, root replication conditions, server/client routes and unreliable saturation; ProcessDemoRecFunction eligibility, client role exchange/recording flags, conditional send and stack-code restoration. Ordered callback-failure propagation preserves legacy no-cleanup-on-throw behavior. C++ retains metadata, script evaluation, method guards and InternalProcessRemoteFunction serialization in `UnScript.cpp`. |
| `actor_audio_contracts` | data-none | fast | `crates/hp2-engine/src/actor_audio.rs`, `crates/hp2-engine/tests/actor_audio_contracts.rs` — Rust-owned listener/view-target selection, strict range and occlusion attenuation, demo-before-propagation ordering, simulated/client viewport versus server pawn routes, PlayOwnedSound-only remote-owner exclusion, IDs/radius parameters, music/sound audio-absence defaults and ordered leaf-failure propagation. C++ retains script decoding, object access/enumeration, line checks, events and audio-subsystem calls behind exception-safe leaves. |
| `actor_spawn_contracts` | data-none | fast | `crates/hp2-engine/src/actor_spawn.rs`, `crates/hp2-engine/tests/actor_spawn_contracts.rs` — Rust-owned spawn/destroy policy, class-resolution/request/result/publication trace order, null-class no-spawn path, post-publication tag assignment, and first-error stop semantics. C++ retains `FFrame` argument decoding/defaults and the engine spawn/destroy/tag leaves. |
| `actor_save_game_exists` | data-none | fast | `crates/hp2-engine/src/actor.rs` — preserves the legacy `FileSize > 0` result, including missing (`-1`), empty (`0`) and positive-size cases; C++ retains save-path construction and `GFileManager` access. |
| `actor_iterator_contracts` | data-none | fast | `crates/hp2-engine/src/actor_iterator.rs`, `crates/hp2-engine/tests/actor_iterator_contracts.rs` — Rust-owned AllActors/ChildActors/BasedActors/TouchingActors/RadiusActors/VisibleActors/VisibleCollidingActors/ZoneActors selection and TraceActors hit-chain copying, class fallback, optional Tag/hidden filtering, owner/Base/zone predicates, collision-expanded and strict radius tests, zero-radius visibility, fixed Touching order, linked hit order and exhaustion. C++ retains FFrame stepping, level actor-array access, IsA/Tag/hidden/FastLineCheck/IsInZone, ActorRadiusCheck and MultiLineCheck/FMemMark services, plus AllActors lifecycle recording. |
| `actor_color_contracts` | data-none | fast | `crates/hp2-engine/src/actor_color.rs`, `crates/hp2-engine/tests/actor_color_contracts.rs` — color-float multiplication operand order, RGB-only scaling, alpha preservation and byte-wrap add/subtract behavior. C++ retains script argument decoding and result-pointer setup. |
| `actor_file_contracts` | data-none | fast | `crates/hp2-engine/src/actor_files.rs` — script object-file/save-info archive policy; missing-file and null LoadGameSaveInfo behavior, ordered first-error propagation without later field writes or success publication. C++ retains path construction, file-manager access, virtual object serialization and typed save-info field access. |
| `actor_file_runtime` | data-prototype | integration | `Tests/AbiTests.cpp` — executes SaveGameSaveInfo/LoadGameSaveInfo through real FFrame expressions, independently decodes the file to check progress-field order and string encoding, restores progress, and checks missing/null loads and byte-sized script boolean results. Artifacts include `save-info.bin`. |
| `object_lifecycle_contracts` | data-none | fast | `crates/hp2-core/src/object_lifecycle.rs` — conditional destroy/register/post-load routing, flags before virtual hooks, reentry, native/exit-purge selection, live object/name tables, complete destroy-before-delete phases and first-error partial state. |
| `object_lifecycle_runtime` | data-prototype | integration | `Tests/AbiTests.cpp` — real UObject hooks, reentrant destruction, throwing Destroy propagation through Rust, and public CollectGarbage deleting two unrooted probes only after both Destroy hooks while preserving a native probe. Writes `object-lifecycle-runtime.json`. |
| `object_gc_contracts` | data-none | fast | `crates/hp2-core/src/object_gc.rs` — live object/name marking, explicit and deferred-localization roots, keep/required masks, reachable cycles, unaligned slots, eliminated references, recursion/context restoration, reference-owner reporting, ignored-slot restoration, delete eligibility and sticky first-error state. |
| `object_gc_runtime` | data-prototype | integration | `Tests/AbiTests.cpp` — real virtual Serialize/FName references, rooted and unrooted cycles, throwing Serialize stopping before purge, IsReferenced owner/ignored-slot behavior, native delete refusal and actual AttemptDelete/GC teardown of six probes. Writes `object-gc-runtime.json`. |
| `decal_contracts` | data-none | fast | `crates/hp2-engine/src/decal.rs` — projection axes and RNG order, square texture-width geometry, BSP coplanar traversal/clipping boundaries, secondary surfaces, matching-texture insertion, duplicate surface ownership, detach and first-error mutation state. Rust retains indices across native array mutations. |
| `decal_runtime` | data-prototype | integration | `Tests/AbiTests.cpp` — real FFrame attach/detach against a deterministic native UModel BSP half-space: disabled/null-texture gates, clipping membership, surface-relative square vertices, MultiDecalLevel clamp, ordered insertion and duplicate ownership removal. Writes `decal-runtime.json`; not visual rendering evidence. |
| `package_map_contracts` | data-none | fast | `crates/hp2-core/src/package_map.rs` — index/support queries, lazy-load order, recursive linker admission, generation negotiation, object/name bases, first-global-name precedence, CopyLinkers virtual dispatch and SerializeName sentinel/error behavior; class-cache early publication, inheritance, admitted-field indices, shared/unmapped condition owners, stable field pointers, native-replication filtering and partial state on first error. |
| `package_map_runtime` | data-prototype | integration | `Tests/AbiTests.cpp` — real Entry linker admission, object/name wire round trips and bounds, copied maps, and Actor/Pawn/PlayerStart class-network caches: inherited field identity, declaration-order indices, shared condition owners, replication lists and Location/Health presence. Writes `package-map-runtime.json`. |
| `path_scout_contracts` | data-none | fast | `crates/hp2-engine/src/path.rs` — Scout selection/spawn, floor placement and iteration limit, testPathsFrom traversal, ValidNode filtering, TestReach location restoration, TestWalk floor support and blocking-normal precedence. C++ retains actor storage and collision/movement/reach/spawn leaves; fallible exports return status separately from query results. |
| `path_scout_runtime` | data-prototype | integration | `Tests/AbiTests.cpp` — real Scout spawning, Entry collision-floor placement and reach query, location restoration/walking physics, path-node hide/show/reset/removal, then full FPathBuilder::buildPaths on a constructed empty level with manual/auto node ownership and Scout teardown checks. Writes `path-scout-runtime.json`. |
| `path_creation_contracts` | data-none | fast | `crates/hp2-engine/src/path.rs` — createPaths/newPath/Pass2From live actor traversal, merge/manual-node ownership, placement/height adjustment, node-array initialization, collision restoration, wall following, anchors, turn/step bounds and first-error stop behavior. Derived navigation fields are read only after class checks. |
| `path_maintenance_contracts` | data-none | fast | `crates/hp2-engine/src/path.rs` — remove/show/hide/undefine live actor-array mutation, counts on refused destruction, exact draw modes, status/log/graph-clear ordering, generated-marker ownership and all persistent navigation links/slots. C++ retains storage, class checks and engine operations. |
| `path_build_contracts` | data-none | fast | `crates/hp2-engine/src/path.rs` — complete active buildPaths phase/timing order, live old-auto-node removal, manual-node preservation, Scout configuration, first-error phase boundaries and graph-allocation failure without publishing an invalid reach-spec index. |
| `class_dependency_archive` | data-none | fast | `crates/hp2-core/src/class_dependency_archive.rs` |
| `property_link_contracts` | data-none | fast | `crates/hp2-core/src/property_link.rs` — property layout/alignment and native offsets, owned/inherited traversal, five linked chains, replication-condition owner equivalence, native constructor binding, network field/index expansion and exact legacy unstable sorting with first-error partial state. Loaded native metadata is exercised by `native_registration` and `package_map_runtime`. |
| `vm_dispatch_contracts` | data-none | fast | `crates/hp2-core/src/vm_dispatch.rs` — ProcessInternal/Event short circuits, remote absorption, singular reentry, return/debug stepping, recursion boundaries, nested timing, copyback-before-local cleanup and live constructor links. C++ retains trace scopes, native frames/alloca and opcode/virtual dispatch leaves. |
| `vm_state_contracts` | data-none | fast | `crates/hp2-core/src/vm_state.rs` — live GotoState/GotoLabel, reentrant End/Begin preemption and first-error flags, auto-state traversal, probe masks, inherited unaligned label tables, sentinels and partial Node/Code publication. Compiled native name IDs cross in the services record, not copied registration numbers. |
| `class_metadata_contracts` | data-none | fast | `crates/hp2-core/src/class_metadata.rs` — function archive 63/64 wire boundary and parameter narrowing, native registered layout tails/alignment, script mismatch and partial-link failures, class default/config/localization precedence and loading-scope errors, inherited state hash ownership and nullable native binding. |
| `core_name_replay` | data-none | fast | `crates/hp2-core/src/name.rs` — Rust-owned name table/free list/hash/init state and intrinsic-name catalog, unchanged native oracle trace, stable entry pointers through growth, LIFO index reuse, exit/reinit and reentrant allocation failure. Native allocator/archive leaves remain in the mixed runtime; the Rust-only package consumer uses concrete OS resources. Oracle generators still use archived headers/source. |
| `object_storage_contracts` | data-none | fast | `crates/hp2-core/src/object_storage.rs` — Rust-owned UObject slots, LIFO free indices and hash heads, stable native object addresses, collision unlinking, live reentrant iteration/cursor publication, shutdown traversal and release/reinitialization. Native allocation, object layout and intrusive hash links remain. |
| `property_archive_contracts` | data-none | fast | `crates/hp2-core/src/property_archive.rs` — live binary/tagged field traversal, pre-76 obsolete values, default suppression, persistent AuxAnims exclusion, legacy fixed-string conversion precedence, malformed-tag skipping and first-error stream/index state. Tag/value policy is also Rust-owned; native archive, reflection and FString conversion primitives remain. |
| `vm_scalar_contracts` | data-none | fast | `crates/hp2-core/src/vm_scalar.rs` — complete bool/byte/int/float operator group, wrapping/narrowing and ARM64 numeric boundaries, short-circuit stepping/error state, interpolation and RNG publication/trace ordering. Native script argument parsing, frames, registrations and trace scopes remain. |
| `property_tag_contracts` | data-none | fast | `crates/hp2-core/src/property_tag.rs` — tag construction/counting, legacy size-code and array-index boundaries (including the 65536 word truncation), struct names, exact/derived bool policy, partially initialized load targets and first-error field publication. Native archive name/scalar primitives, trace scopes and the counting archive remain. |
| `property_value_contracts` | data-none | fast | `crates/hp2-core/src/property_value.rs` — active property copy/compare/destroy and binary/network serialization, inherited/constructor-chain traversal, live versus captured dimensions/strides, forward-overlap copies, metadata mutation, compressed vectors/rotators/planes and first-error publication. Native storage, archive/package-map primitives and reflection/virtual calls remain. |
| `vm_vector_contracts` | data-none | fast | `crates/hp2-core/src/vm_vector.rs` — complete vector/rotator operator group, ARM64 contraction and integer conversion, wrapping rotations, aliased assignments/axes, rejection sampling and first-error RNG/result ordering. Native argument decoding, frames, registrations and trace scopes remain. |
| `property_text_contracts` | data-none | fast | `crates/hp2-core/src/property_text.rs` — tokenization, property import/export and C++ declarators, scalar/enum cursor quirks, quoted text, localized booleans, object/class rejection, inherited/array fields, live storage, round trips and first-error partial mutation. Native FString/array storage, reflection, virtual calls and variadic formatting remain. |
| `vm_convert_contracts` | data-none | fast | `crates/hp2-core/src/vm_convert.rs` — all 33 scalar/object/name/string/vector/rotator conversions plus dynamic/metaclass casts, signed arm64 float-byte narrowing, low-bit versus nonzero booleans, localized precedence, comma parsing, aliased inputs and atomic publication after parse/query errors. Native storage, lookup, argument decoding and formatting primitives remain. |
| `vm_string_contracts` | data-none | fast | `crates/hp2-core/src/vm_string.rs` — all 21 string/name/object operators, counted versus terminated substrings, unsigned Mid bounds, asymmetric concatenation storage, host-locale comparison, UTF-32 indexing, ASCII Caps, full-width pointer identity and resize/error boundaries. Native argument ownership, FString representation and allocator leaves remain. |
| `vm_variable_contracts` | data-none | fast | `crates/hp2-core/src/vm_variable.rs` — all 16 variable/array/context/assignment/struct-comparison opcodes, live GProperty/GPropAddr, clamping/growth, bool target snapshots, null-context skipping, object lookup/cursor errors and live constructor destruction. Native frame layout, alloca, storage/virtual calls and trace leaves remain. |
| `vm_flow_contracts` | data-none | fast | `crates/hp2-core/src/vm_flow.rs` — all 22 flow/debug/constant opcodes, unaligned operand/result widths, four-byte object/name indices, switch traversal/string lifetime, branch trace/reentry, debug rewind and first-error cursor/result state. Native scratch, debugger/trace/lookup/formatting leaves and runaway guards remain. |
| `vm_call_contracts` | data-none | fast | `crates/hp2-core/src/vm_call.rs` — native/remote/script CallFunction routing, absorbed argument/debug consumption, live parameter/constructor links, 16 stack-owned out records, reverse aliased copyback, captured observer lifetime and first-error partial locals/outputs. Native FFrame/alloca, profiling/trace scopes and invocation/storage primitives remain. |
| `vm_object_contracts` | data-none | fast | `crates/hp2-core/src/vm_object.rs` — all 13 object/state/query/property-text/enum/load/RandRange native policies, compiled probe masks, live inheritance, transition preemption, early enum NAME_None publication, captured enum-length assertions and first-error output/RNG/trace order. Native construction/load/storage/format/trace leaves and argument/lifetime scopes remain. |
| `vm_registry_contracts` | data-none | fast | `crates/hp2-core/src/vm_registry.rs` — Rust-owned GNatives backing, initialization and duplicate state; INDEX_NONE initialization, duplicate overwrite, opaque two-word member-pointer equality, all high prefixes/byte boundaries and live cursor consumption. Native typed member invocation and ABI assertions remain; registration is serialized. |
| `vm_frame_contracts` | data-none | fast | `crates/hp2-core/src/vm_frame.rs` — critical/nonzero-strict routing, live parent/node/root-object diagnostic traversal, skipped-node separators, UTF-32 scratch, first-error partial text and opcode consumption before aliased/reentrant native dispatch. Native FFrame representation, static scratch, formatting/invocation primitives and guard scopes remain. |
| `array_storage_contracts` | data-none | fast | `crates/hp2-core/src/array_storage.rs` — raw buffer set/exact replacement/insert/add/remove/shrink/release policy, native growth/slack thresholds, unaligned pointer slots, live allocator reentry, count-before-pointer publication, size/error boundaries and zero-width/null moves. Native packed ABI, typed construction/destruction and allocator leaves remain. |
| `object_collections_contracts` | data-none | fast | `crates/hp2-core/src/object_collections.rs` — actual Rust root/loader/loaded/registrant backings, duplicate order/removal, live registrant growth, suffix sort, partial errors, root archive count/load/save and immediate slot-write visibility before callback growth. Serialized native engine-thread access and no use of a slot after callback growth are required. |
| `string_storage_contracts` | data-none | fast | `crates/hp2-core/src/string_storage.rs` — implemented FString construction/assignment, counted versus terminated values, append/path, unsigned ranges, directional search, live aliased Split, native case mapping, padding/capacity and Chr; guarded noinit self-identity, allocator reentry and first-error publication. Native packed representation, typed lifetimes, allocator/case/diagnostic/formatting leaves remain. |
| `map_storage_contracts` | data-none | fast | `crates/hp2-core/src/map_storage.rs` — TMap/TMultiMap collision chains, hash growth/relaxation, replacement/unique pairs, reverse stable removal, MultiFind ordering, archive rehash, generic typed queries, packed slots, reentry and partial-error publication. Native typed pair construction/destruction, hashing/comparison and archive leaves remain. |
| `object_properties_contracts` | data-none | fast | `crates/hp2-core/src/object_properties.rs` — Rust default-source choice/re-read, byte copy/zeroing, live constructor-property traversal, offset/range checking, typed destruction sequencing, first-error diagnostics and class-default header/superclass orchestration. Native object-header layout, virtual property copy/destruction and guard leaves remain. |
| `object_construction_contracts` | data-none | fast | `crates/hp2-core/src/object_construction.rs` — Rust validation/name/conflict/replacement policy, allocation/reservation, flag/config restoration, property initialization/publication and constructor sequencing. Native typed layout, constructors/destructors, memory tags and diagnostic leaves remain. |
| `package_file_contracts` | data-none | fast | `crates/hp2-core/src/package_file.rs`, `crates/hp2-core/tests/package_file_contracts.rs` — Rust-owned `appFindPackageFile` policy: DLL rejection, whole-input normalization (UE1's 255-unit truncation could match a different file), candidates composed in owned strings (no 256-unit scratch limit), caller-sized `Out` with logged not-found for hits that do not fit, direct-file precedence, configured/CD path and localized/default candidate order, cache GUID/timestamp, case-insensitive fallback, final-matching-filesize quirk, output-on-miss and ordered service failure. C++ retains `GSys`, file-manager/enumerator, cache and diagnostic leaves. |
| `file_persistence_contracts` | data-none | fast | `crates/hp2-core/src/file_persistence.rs`, `crates/hp2-core/tests/file_persistence_contracts.rs` — Rust-owned binary/text file load/save policy, ANSI conversion, UTF-16 LE/BE validation and decoding, UTF-16LE BOM/scalar encoding, malformed-input destination preservation, size bounds, writer error/Close/delete ordering and first-error propagation. Binary operations retain their distinct legacy error semantics; C++ retains archive virtual dispatch and typed native storage/conversion leaves. |
| `config_archive_runtime` | data-prototype | integration | `Tests/AbiTests.cpp` — actual dirty `FConfigCacheIni` destruction through a faulting native file writer. The original INT7301 reaches its caller only after the archive is destroyed exactly once, with zero live writers before fixture cleanup; normal readback, fresh write/read recovery and global file-manager restoration are also required. |
| `file_stream_contracts` | data-none | fast | `crates/hp2-ffi/src/core_file_stream.rs` — Rust-owned worker/lifecycle, fixed-address stream storage, bounded request/completion rings and regular/Ogg/XA resource/read policy. Contracts cover full-capacity duplicate destinations through wraparound, Leave during shutdown, and source failure slot recovery. Native codec/lifecycle integration is covered by `audio_lifecycle`. |
| `mem_stack_contracts` | data-none | fast | `crates/hp2-core/src/mem.rs`, `crates/hp2-core/tests/mem_stack_contracts.rs` and existing `mem_replay` — Rust PushBytes alignment/current-chunk/growth and explicit FMemMark capture/Pop, zero-size/exact-fit boundaries, live-byte preservation, first-fitting chunk reuse, unchanged same-chunk End, and allocation/size/slow-check failure publication. Native stack/mark layouts, allocator and guard leaves remain; mark destruction intentionally does not Pop. |
| `linker_runtime_contracts` | data-none | fast | `crates/hp2-core/src/linker_runtime.rs` — Rust linker load/save setup, fullnames and lookup, import verification/legacy compatibility, recursive export publication, preload flag/position/error order, lifecycle, lazy-loader and object archive policy. Contracts cover compatibility hashing/parents, recursive map growth and mutable-versus-const text cleanup. Native typed fields/constructors, archive/engine leaves, guard and stack-only tag scopes remain. |
| `math_inline_contracts` | data-none | fast | `crates/hp2-core/src/math.rs` — Rust-owned active native inline scalar/vector/plane/coords/rotator/quaternion/box/geometry policy, aliased updates, clipping short-circuit behavior and native AArch64 floating-point operand/FMA order. Contracts include reciprocal NaN priority, signaling/quiet clamp behavior, shear invalid products and quaternion NaN signs. Rust owns canonical tables and directly addressable WorldMin/WorldMax/UnitCoords/UnitScale/ViewCoords storage; no native FGlobalMath class or GMath instance remains. Native typed ABI declarations and random/platform leaves remain; compiler-constant-folding NaN-sign limits are recorded in `ORACLE_BASELINE.md`. |
| `array_archive_contracts` | data-none | fast | `crates/hp2-core/src/array_archive.rs` — Rust typed-array archive traversal/destruction, transaction mutation/undo order, lazy version/backpatch/load/unload and memory-buffer growth/cursor/copy policy. Native typed element lifetimes/storage, virtual archive calls and guard/assert leaves remain. |
| `package_flow_contracts` | data-none | fast | `crates/hp2-core/src/package_flow.rs` — Rust object/class lookup, name/config resolution, package/linker/object/class loading, nested load/preload/postload/reset sequencing, recursive tagging and conformal save/mapping/sort orchestration. Contracts include 900-character configuration sections, live/error ordering, and text that does not fit the native filename/partial-name buffers (`text_capacity`, 1024 = PATH_MAX): unresolved name without a copy, `SaveWarning` before any save work; native typed archives, lifetime/exception/comparator/platform leaves remain. |
| `runtime_init_contracts` | data-none | fast | `crates/hp2-core/src/runtime_init.rs`, `crates/hp2-core/tests/runtime_init_contracts.rs` — Rust startup/teardown and persistent actor-cache orchestration; executable-prefix quote/whitespace parsing, UTF-16/UTF-32 character offsets and zero-versus-negative file-size selection. Actual lifecycle, file/cache operations and first-error identity/order have separate native differential evidence in `ORACLE_BASELINE.md`. Native globals, typed lifetimes, allocator/platform/config/file and diagnostic leaves remain. |
| `core_net_archive` | data-none | fast | `crates/hp2-core/src/core_net_archive.rs` |
| `dxt1_codec` | data-none | fast | `Tests/Dxt1Tests.cpp` |
| `eaxa_decoder` | data-none | fast | `Tests/EaxaTests.cpp` |
| `engine_path_define_paths` | data-none | fast | `crates/hp2-engine/src/path.rs` |
| `engine_actor_anim` | data-none | fast | `crates/hp2-engine/src/actor_anim.rs` — includes animation selection, channel reuse, and Replace/Combine subset cleanup |
| `framing_baseline_contract` | data-none | fast | `Tests/FramingBaselineTests.py` |
| `fstring_archive` | data-none | fast | `Tests/AbiTests.cpp` |
| `ftime_archive` | data-none | fast | `Tests/AbiTests.cpp` — native eight-byte FTime representation and archive compatibility; scalar arithmetic/conversion policy is Rust-owned in `crates/hp2-core/src/time.rs`. Complete native-consumer differential evidence is recorded below. |
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
| `package_summary_archive` | data-none | fast | `crates/hp2-core/src/package_summary_archive.rs` |
| `linker_name_archive` | data-none | fast | `crates/hp2-core/src/linker_name_archive.rs` |
| `linker_record_archive` | data-none | fast | `crates/hp2-core/src/linker_record_archive.rs` |
| `projection_fov` | data-none | fast | `Tests/AbiTests.cpp` |
| `procedural_texture_runtime` | data-prototype | integration | `Tests/AbiTests.cpp` — loads real Engine.u/Fire.u, checks script/native procedural field offsets, constructs and tears down all four concrete texture families, preserves exact FSpark wire order and truncated-load state, and propagates the original archive INT fault before successful recovery. |
| `prototype_archive_contract` | data-none | fast | `Tests/PrototypeArchiveTests.py` |
| `render_clip` | data-none | fast | `Tests/AbiTests.cpp` |
| `renderer_smoke_xopengl` | data-prototype | integration | `Build/smoke_maps.py` |
| `repair_save_contract` | data-none | fast | `Tests/RepairSaveTests.py` |
| `replay_roundtrip` | data-none | fast | `Tests/ReplayRoundTripTests.cpp` |
| `rng_seed_contracts` | data-none | fast | `Tests/RngSeedTests.cpp` — real native consumer of Rust diagnostic seed parsing/bootstrap: strict decimal/RAND_MAX boundaries, unchanged outputs on absent/invalid input, endpoint normalization and actual libc rand() stream equivalence; also output/input aliasing and caller-specific 15/32-bit limits. |
| `save_format_contract` | data-none | fast | `Tests/SaveFormatTests.py` |
| `spell_interaction_manifest` | data-prototype | integration | `Build/spell_interaction_audit.py` |
| `spell_runtime_contracts` | data-prototype | integration | `Tests/SpellRuntimeTests.cpp` |
| `ucc_help_smoke` | data-prototype | integration | `Tests/UccSmokeTests.py` |
| `ucc_smoke_contract` | data-none | fast | `Tests/UccSmokeTests.py` |
`Class` is `fast` for a contract that runs in seconds and `integration` for one
that launches the engine or walks a full map.


`compact_index` covers signed 32-bit boundaries, compact-length transitions,
canonical round trips, and truncated archive reads.

`fstring_archive` pins the compact-index ANSI form and UTF-16LE wire form,
including high-byte ANSI, supplementary scalars, malformed surrogate
replacement, invalid host scalar replacement, and truncated archive errors.
`ftime_archive` pins the legacy 32-bit-float bitfield layout inside the
byte-ordered 64-bit archive field, including negative values and truncated reads.
`package_summary_archive` covers version 68 generation records and the legacy
pre-68 heritage-GUID seek/restore path. Legacy saves now write zero heritage
count/offset values instead of the former uninitialized C++ locals.
The standalone `FGenerationInfo` and `FGuid` operators use Rust serializers;
the manual `hp2_save_wire_oracle` checks their little-endian field order and
compares the emitted package-79 wire to
`Tests/Fixtures/save-wire/cxx-save-wire-v1.usa`.
`class_dependency_archive` keeps `FDependency` wire order (`Class`, `Deep`,
`ScriptTextCRC`) while delegating object-reference mapping and archive byte
order to the active C++ `FArchive`.
`linker_name_archive` covers both linker FName archive directions: saving
maps the global name index to the package-local compact index, and loading
resolves valid package-local indices while rejecting negative and one-past-end
indices through the legacy `Bad name index` error path.
`linker_record_archive` pins export/import record field order, conditionally
serializes export offsets only for nonzero serialized size (including the
load-time size transition), and clears the import's runtime-only object/source
references after a load.
`package79_manifest` additionally writes real `FObjectExport` and
`FObjectImport` records through `ULinkerSave` with a package-local FName
mapping, then decodes every field through `FArchive`; this covers the save
linker's `FArchive` base adjustment, record field order, and that saving does
not clear import runtime-only fields.
`engine_path_define_paths` exercises the active `FPathBuilder::definePaths`
graph builder reached by `PATHS BUILD`: eligible same-class navigation points
create special reach specs, `PlayerStart` origins are excluded, and the
navigation-point list is cleared. Rust also owns scout selection, floor
placement and reach-query policy covered by `path_scout_contracts`, and
`createPaths`/`newPath`/`Pass2From` policy covered by `path_creation_contracts`.
Rust also owns navigation maintenance covered by `path_maintenance_contracts`
and complete active `buildPaths` orchestration covered by `path_build_contracts`.
Graph allocation reports exception status separately from the allocated index.
The native fixture executes the full builder API on a constructed empty level;
it is not evidence of the visible editor command or a populated-level wall-tour
build. Entry floor/reach queries are exercised separately.
Lift and visibility helpers belong only to the dormant alternate builder and are not
active path-build behavior.
Rust now owns name and UObject slot/free/hash storage, native-opcode registry
backing/initialization/duplicate state, raw array allocation/growth/move/shrink/
release policy, actual root/loader/loaded-object/registrant collection backings,
implemented FString storage/value and TMap/TMultiMap hash/mutation policy,
UObject default-property initialization/destruction/class-default orchestration
and allocation/replacement/construction policy, GC root/mark/reference/delete
policy, reflection layout/chains/metadata-serialization and binding policy,
UStruct binary/tagged property traversal, tag construction/wire/bool policy,
active property value copy/compare/destroy and binary/network algorithms,
property tokenization/text import/export/C++ declarator policy, VM function/
event/state/label control flow, CallFunction routing/parameter/copyback/
destruction, variable/context/assignment/struct-comparison and flow/debug/
constant opcodes, all conversion groups and scalar/vector/rotator/string/name/
object operator groups, object/state utility-native policy, and FFrame
diagnostic traversal/severity/opcode consumption. The full cutover remains
incomplete: C++ retains UObject typed layout/allocation leaves, property value
storage and virtual operations, VM frame representation and typed member
invocation, native container representations and typed constructors/destructors,
legacy empty FString numeric constructors, other engine collections,
allocator/case/formatting leaves, and platform objects.
Rust-owned container and UObject policy do not complete native-storage removal.

`canvas_compatibility_contracts` also round-trips native-font and page-backed
`UFont` objects through archive versions 68 and 69, covering the legacy
remapping cutoff and glyph-table serialization.

## Native policy cutover checkpoint

The integrated `native-scene-paths-text-cutover` checkpoint moves object bootstrap,
diagnostic console, object preference/driver-cache/language registry, scene
BSP/clipping/frame/draw/decal/volumetric orchestration, bootstrap path/identity,
and native text layout/raster policy into Rust. C++ retains typed engine and
platform leaves; this is not a full-Rust runtime.

Native before/after probes compare complete ordered records, including failures:

| Surface | Exact records |
|---|---:|
| Object bootstrap, including manager/logger faults | 117 |
| Diagnostic console | 41,510 |
| Object preference/cache/language registry | 235 |
| Scene pipeline | 381 |
| Bootstrap paths and package identity | 212 |
| CoreText layout and glyph raster bytes | 11,455 |
| FreeType layout and glyph raster bytes, macOS-host shim | 11,787 |

The path comparison additionally matches all 54,220 merged output bytes,
including malformed UTF-8 diagnostics. Native filesystem calibration and the
registered path regression cover unresolved directory terminators. CoreText and
FreeType comparisons include glyph metrics and complete alpha bytes; the
FreeType host shim is not Linux runtime evidence.

Observed verification on macOS arm64: the verification binaries and signed app
build passed; `cargo test --offline -p hp2-core -p hp2-render -p hp2-launcher --lib`
passed 204 tests; the named CTest selection passed all 61 targets (53 data-none,
eight data-prototype), including `paths_bootstrap_contracts`; bundle verification
passed under data-none. The original compiler-cache IPC failure and the corrected
short-alias environment/socket smoke remain in the acceptance evidence.

The rebuilt native game completed 120 `Maps/Adv8Forest.unr` ticks under
data-prototype with exit 0, draw submissions, and normal final object purge.
All 120 frames were occluded: this is not visible gameplay acceptance. No
physical-footprint sample, retail overlay verification, retail runtime, or Linux
runtime backs this checkpoint. The user subsequently reported the 16 GB issue
resolved; this checkpoint does not establish its cause. Feature gates are
unchanged. The dynamic-node checkpoint below supersedes this checkpoint's
renderer ownership limitation. Native UObject/property/frame/container ABI
ownership remains separate full-port work.

Exact commands, ordered comparison results, initial failures, runtime observations,
and unresolved conditions:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-scene-paths-text-cutover/acceptance.json`.

## Native dynamic, property-offset and LOD ownership checkpoint

The subsequent in-place cutover moves the native property-offset registry and
load policy, LOD archive/remap traversal, and dynamic renderer node/child ownership
into Rust. Native renderer nodes are nonowning typed views; reusable Rust slabs
and nested scopes own their storage. The C++ SDK object layout, typed array
lifetimes, raster/span primitives, and device submissions remain native seams,
not evidence of a completed full Rust port.

Complete ordered native comparisons passed on macOS arm64 under data-prototype:

| Original consumer oracle | Exact matching records |
|---|---:|
| Property registration, full capacity, archive versions, reentry, and failure boundaries | 115 |
| LOD save/load/count, packed remap bytes, lazy loading, and archive/allocation failures | 406 |
| Dynamic setup/filtering, equal-depth order, child drawing, spans, volumetrics, and strict light boundaries | 217 |

Twelve additional native records verify distinct nested child owners, reusable
storage after normal/error scope exit, and four span-leaf failure/recovery
boundaries. Mixed native chunk/light depth fields were uninitialized; the
original oracle explicitly initializes those fields. The new shell-only depth
is a defined replacement for native undefined behavior, not claimed parity.

The injected standalone DrawActor device failure exposed an existing Canvas void
callback that aborted across Rust. Rust now returns the complete icon draw
request; C++ submits it after Rust returns. The rebuilt native scenario catches
the actual device exception, performs the next draw successfully, and retains
all ordinary original records. The original abort is retained as failure evidence.

Observed verification: 194 Core/Engine/Render library tests plus the existing
Canvas draw-call reference test passed, as did all 61 selected named CTest targets
(53 data-none and eight data-prototype) and data-none signed-bundle verification.

```sh
cargo test --offline -p hp2-core -p hp2-engine -p hp2-render --lib
cargo test --offline -p hp2-engine --test canvas_draw_calls
```

The rebuilt game completed 120 `Maps/Adv8Forest.unr` ticks with exit 0, draw
submissions, and a final object purge from 23536 to zero. Four frames were
presented and 116 occluded; no visual capture or interactive acceptance backs
this run. No retail or Linux runtime was exercised, no feature gate was changed,
and the user-resolved RAM incident was not reopened or attributed to this port.

Exact commands, source snapshots, original failures, complete comparisons, and
runtime observations:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-dynamic-offset-lod-cutover/acceptance.json`.

## Native cache, light-manager and LOD rendering checkpoint

Rust now owns the remaining cache lookup/locking/checked-size policy, the light
manager's authoritative state and setup lifetime, and complete LOD draw
orchestration. Native SDK layout, typed mipmap construction/destruction, texture
locks, and render-device submissions remain explicit interop leaves. Shared
mesh batching/clipping and ordinary drawing were still native at this checkpoint;
their subsequent cutover is recorded below.

Observed native comparisons on macOS arm64 under data-prototype:

| Consumer oracle | Exact evidence | Reference provenance |
|---|---:|---|
| Ordinary LOD geometry, shading, texture selection, state and error prefixes | 521 records | Original pre-rebuild executable |
| Callback-mutated actor/frame/texture state and skeletal error context | 325 records | Frozen original LOD policy with rebuilt shared helpers |
| Cache lookup, MRU, misses, lock costs and checked sizes | 131 records | Frozen original inline policy with defined fixture inputs and rebuilt shared helpers |
| Light-manager state, tables, actor/surface output and failure recovery | 57 records | Original pre-rebuild executable |
| Two fog passes, excluding unwritten row padding | 48 written bytes | Debugger captures from retained original and rebuilt executables |

The initial cache fixture read uninitialized time/profiling fields; its
differences remain recorded. The corrected fixture explicitly seeds those
inputs before initialization, without changing production initialization.
The initial two fog hashes included eight unwritten padding bytes per pass.
Direct captures show that only texels 3 and 7 differ in each 4-by-2 allocation;
all six defined texels per pass match. Neither original result was silently
normalized, and no production zeroing was added.

The injected wire-device exception previously aborted across the camera's
Rust callback. Camera projection now returns a line/point plan, and native
submission happens after Rust returns. The rebuilt scenario catches the
exception and immediately performs the next wire draw successfully: 14 device
submissions, no pending cache unlocks. Ordinary camera reference cases remain
unchanged.

Observed verification: 231 distinct Rust behavior tests, all 61 selected named
CTest targets (53 data-none and eight data-prototype), and data-none signed-bundle
verification passed. Fixture-count-only tests were removed; behavior replays
were retained and rerun.

The rebuilt game completed 120 `Maps/Adv8Forest.unr` ticks with exit 0, 2275
triangle and 5030 complex-surface calls, and a final object purge from 23536 to
zero. All 120 frames were occluded; this is execution evidence, not visual or
interactive acceptance. No retail/Linux runtime or feature-gate promotion is
claimed. The user-resolved RAM incident remains closed.

Exact commands, source snapshots, initial failures, fixture corrections,
debugger byte captures and unresolved conditions:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-cache-light-lod-render-cutover/acceptance.json`.

## Native object serialization and shared mesh rendering checkpoint

Rust now owns the complete `UObject::Serialize` archive policy, shared mesh
clipping/indexed batching/remapping, texture/environment/UV/unlit state,
special-coordinate symbols, and ordinary non-LOD draw orchestration. LOD
submission uses the same authoritative Rust state directly. Native SDK field
views, typed lifetimes, virtual calls, timing and exception guards remain leaves.

Observed native comparisons on macOS arm64 under data-prototype:

| Consumer oracle | Exact ordered records |
|---|---:|
| Archive modes/chaining, live object/frame state, wire bytes and failure prefixes | 2312 |
| Shared clipping/batching and ordinary mesh rendering | 1195 |
| Callback-mutated mesh/actor/frame/texture state, nested batching and recovery | 677 |
| Existing LOD geometry, shading, textures, state and failures | 521 |
| Existing LOD callback mutation and failure cases | 325 |
| LOD wire-device exception and immediate recovery | 18 |

Every original reference executable was compiled, linked and run before the
first production rebuild. Interpolation leaves native clip-vertex Normal/Flags
unwritten; only those fields remain separate raw diagnostics. The first
mutation fixture incorrectly classified six defined outer vertices flushed
during nested batching; its corrected pre-rebuild capture retains them.
Original output remains recorded; production memory was not zeroed.

Review found that direct camera planning bypassed virtual `Draw3DLine`
overrides. An actual SDK consumer reproduced zero override calls and no
injected exception, exiting 91. Preserving the virtual SDK leaf restores
15 calls, the injected exception, and 15 successful immediate-recovery calls,
exiting 0. Its base camera algorithm remains Rust; the unused direct engine
dependency was removed. Failing/passing consumers and executables are retained.

Observed verification: 231 Rust behavior tests, all 61 selected named CTest
targets (53 data-none/eight data-prototype), and data-none signed-bundle
verification passed. The rebuilt game completed 120 `Maps/Adv8Forest.unr`
ticks with exit 0, 2274 triangle and 5030 complex-surface calls, and a final
object purge from 23536 to zero. All frames were occluded: no visual or
interactive acceptance, retail/Linux runtime, or feature-gate promotion is
claimed. The RAM incident remains closed; the full Rust port remains active.

Exact commands, original sources/results, fixture corrections, regression
reproduction, final comparisons and unresolved conditions:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-object-serialize-shared-mesh-cutover/acceptance.json`.

## Rust mesh dispatch checkpoint

`URender::DrawMesh` now delegates the LOD-versus-ordinary branch to
`hp2_rs_render_mesh_dispatch`. Rust queries the typed C++ `ULodMesh` predicate,
selects exactly one rendering leaf, and stops on predicate or selected-leaf
failure. C++ retains only typed mesh/renderer invocation, native exception
transport, SDK guards, and the existing Rust-owned ordinary/LOD pipelines.

Observed on macOS arm64: the three Rust dispatch contracts passed, the signed
data-none bundle passed integrity checks, and the rebuilt WGPU app completed
120 `Adv8Forest` ticks under data-prototype. The smoke is not foreground visual
proof and does not cover Linux or retail data.

## Rust renderer precache checkpoint

Rust owns `URender::Precache` model-surface and global-texture traversal order,
masked/no-smooth flag propagation, and parametric-texture exclusion. C++ keeps
live object iterators, texture lock/unlock, render-device precache, formatted
guard paths, and exception transport as typed leaves.

The Rust traversal contract passed and the rebuilt WGPU app completed 120
`Adv8Forest` ticks under data-prototype. This is not a foreground visual proof
and does not cover Linux or retail data.

## Rust renderer console checkpoint

Rust owns `URender::Exec` command grammar, ordered `STAT` toggle parsing,
`REND` selection, and scalar-command dispatch for `TLOD`, `MLOD`, `MLMODE`,
and `MLFIX`. C++ retains typed renderer-field writes, UE1 scalar parsing,
output logging, SDK guards, and exception transport.

Three Rust console contracts and a native constructed-`URender` CTest passed,
covering field writes, handled results, and output logs through the C++ → Rust
→ C++ boundary. The signed data-none bundle passed integrity checks and the
rebuilt WGPU app completed 120 `Adv8Forest` ticks under data-prototype. This
does not exercise interactive console entry, provide foreground visual proof,
or cover Linux or retail data.

## Rust renderer pre-render checkpoint

Rust owns `URender::PreRender` leaf sequencing: time rollover, count reset,
memory-mark acquisition, fast-math enablement, and random tick admission. C++
retains typed engine calls and guard/exception transport.

Two pre-render contracts passed; the signed data-none bundle passed integrity
checks and the rebuilt WGPU app completed 120 `Adv8Forest` ticks under
data-prototype. This is not foreground visual proof and does not cover Linux or
retail data.

## Rust renderer statistics checkpoint

Rust owns `URender::DrawStats` numeric prelude; FPS; global-panel timing and
decal conversion; network channel counting, truncating display conversion, and
rate-saturation selection; mesh, particle, EARI summary/detail, and occlusion
panel cycle-to-millisecond conversion; mesh LOD division; particle zero-count
guards and integer area conversion; occlusion raster-attempt aggregation; and
canvas-panel dispatch order (color reset; FPS; shared stats cursor;
global/network panels; network-state commit; hardware, mesh, particle, zone,
occlusion, game, cache, EARI summary/detail, and animation cursor). It
preserves global client-minus-blit and platform-absolute timing, render
conversion order, decal subtraction, particle-render adjustment, conditional
averaging, negative-average RMS mode, missing-EARI entry `99.99` sentinels,
legacy float/double conversion stages, and total time progression. C++ retains
native clock/RNG leaves and typed viewport, connection and URL lookup, network
log, level-stat text, canvas, Legend, font, EARI-name/tag arrays, and
render-device output.

Eleven numeric/FPS contracts, two dispatch contracts, and the native C++→Rust
contract passed. The rebuilt WGPU app completed 120 `Adv8Forest` ticks under
data-prototype; latest report:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/UnRender/occlusion-stats-wgpu.json`.
This does not exercise requested statistics overlays, provide foreground visual
proof, or cover Linux or retail data.

## Rust renderer lifecycle checkpoint

Rust owns `URender::Init` and `URender::Destroy` sequencing and their default
renderer state. C++ retains typed memory/cache/light-manager calls, base-class
calls, global/field writes, logging, and guard/exception transport.

Two lifecycle contracts passed; the signed data-none bundle passed integrity
checks and the rebuilt WGPU app completed 120 `Adv8Forest` ticks under
data-prototype. This is not foreground visual proof and does not cover Linux or
retail data.

## Rust renderer post-render checkpoint

Rust owns `URender::PostRender` adaptive mesh and particle-detail adjustment:
frame-rate clamping, mesh LOD adjustment, particle-time/area branch selection,
and final particle-size clamping. It owns Legend actor mesh-light-count
selection and PostRender frame/process timing: frame caps, integer stat
conversion, process-baseline transitions, and external-time updates. C++ reads
native clocks and fields, performs checks, and writes the live renderer state.

Nine boundary contracts passed; the signed data-none bundle passed integrity
checks and the rebuilt WGPU app completed 120 `Adv8Forest` ticks under
data-prototype. The configured macOS build excludes `LEGEND`, so the
`ComputeActorMeshLighting` C++ → Rust bridge remains unexercised. This is not
foreground visual proof and does not cover Linux or retail data.



## Rust dynamic-renderer admission checkpoint

Rust owns the top-level and complete per-actor admission policy for
`URender::SetupDynamics`: nonempty-BSP/show-actors gating, actor-array
`First`/`IsDone`/visit/`Next` traversal, high-detail and recursion/view-target
filtering, exclusion/hidden/visibility checks, radius and height checks, and
both owner-visibility rules. Its C++ service calls are requested lazily in the
original short-circuit order, preserving repeated `IsOwnedBy` calls and
stopping immediately on a native exception.

Rust also owns moving-brush versus ordinary-sprite route selection, ordinary
sprite versus iterator versus filtered-iterator draw-route selection (reading
the filter only after iterator `Init`), render-iterator `First`/`IsDone`/
visit/`Next` traversal order, render-interface construct/destroy/retain
selection and checked lifecycle sequencing (construct, or conditional destroy →
delete → clear the live slot), dynamic-light eligibility, shadow-projection box and
behind-projection arithmetic, shadow admission ordering, and shadow-outcome
trace/update dispatch selection. `IsMovingBrush`, brush-tracker updates,
iterator lifecycle/cursor operations, candidate drawing, `FBox` construction,
`BoundVisible`, native trace recording, shadow event update, SDK object
operations, and statistics remain checked typed C++ leaves.

The native C++→Rust contracts passed, and the rebuilt WGPU app completed 120
`Adv8Forest` ticks under data-prototype:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/UnSprite/iterator-lifecycle-dispatch-wgpu.json`.
This is not foreground visual proof and does not cover Linux or retail data.


## Native object instance and portable texture runtime checkpoint

Rust now owns `UObject` property export, execution initialization, probe masks,
script-console dispatch, field/function/state lookup, and authoritative pending
localization storage. The native pending-localization array and its queue
callbacks are removed. Archive slot publication, allocation accounting, nested
queue mutation and failure prefixes retain the original SDK behavior.

Portable texture defaults, timing, virtual lock/update policy, mip archives,
palette reuse, mipmap orchestration, lazy data lifecycle, snapshots, BMP/PCX
import and animation linking now execute in Rust. SDK fields, typed arrays and
lifetimes, virtual calls, archives, filesystem operations and error formatting
remain native leaves; existing Rust color and mip kernels are reused.

Observed native comparisons on macOS arm64 under data-prototype:

| Consumer oracle | Exact ordered records |
|---|---:|
| Object instance lookup/execution/export and live pending-localization storage | 410 |
| Portable texture/color/mip/archive/timing/import lifecycle | 3995 |
| Defined early-mismatch reuse of one-, two- and three-color palettes | 4 |

Original sources, headers and complete SDK link inputs were retained before
editing; every original consumer was compiled, linked and run before rebuilding.
The first build caught legacy name-registration IDs mistaken for compiled enum
ordinals; the next caught native payload-width conversions. Both source fixes
retain explicit compile-time checks, and the failed build logs remain recorded.
Earlier fixture compile/setup and inherited-overlay corrections are also retained.

Observed verification: 231 Rust behavior tests, all 61 selected named CTest
targets (53 data-none/eight data-prototype), and data-none signed-bundle
verification passed. The rebuilt game completed 120 `Maps/Adv8Forest.unr`
ticks with exit 0, 2276 triangle and 5030 complex-surface calls, and a final
object purge from 23536 to zero.

All frames were occluded: no visual or interactive acceptance is claimed.
Compressed/deferred-render-device `Lock` branches are migrated but lack a
consumer oracle in this checkpoint. Malformed unsafe image inputs and inactive
MSVC codecs are excluded; portable unsupported compression remains unchanged.
No retail/Linux runtime or feature-gate promotion is claimed. The RAM incident
remains closed; the full Rust port remains active.

Exact commands, source snapshots, before/after comparisons, preserved failures
and unresolved conditions:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-object-instance-texture-runtime-cutover/acceptance.json`.


## Native span clone and mesh lifecycle checkpoint

Rust now owns complete `FSpanBuffer` copy construction and diagnostic traversal,
including allocation/mark publication order, live node and row-bound reads,
returning/reentrant error-device behavior, and failure-prefix propagation.
Stack allocation, marks and checked-size conversion now call the existing Rust
memory owner directly. Diagnostic formatting and SDK storage remain typed leaves.
Four-byte node allocation requests are preserved; pointer-link
stores use unaligned writes rather than assuming eight-byte alignment.

Rust also owns explicit ordinary/LOD mesh constructor defaults, array growth
and texture clearing, lazy frame preparation, post-load state capture, and
successful LOD-result publication. Existing Rust frame kernels are reused.
Native array element lifetimes, archive operations, cache access, virtual
calls and checked exception transport remain SDK leaves.

Observed original-versus-rebuilt SDK comparisons on macOS arm64 under
data-prototype:

| Consumer oracle | Exact ordered records |
|---|---:|
| Span copy, diagnostics, live mutations, allocation failures and recovery | 210 |
| Mesh constructors, lazy archives, frames, partial failures and recovery | 4471 |

The complete observable intervals, including unprefixed error-history
continuations, matched without normalization. Original sources and complete
SDK linkage were retained before rebuilding. Initial fixture include/lifetime
failures and a build failure from missing per-enum cbindgen prefixing are
retained with their corrections; no unchanged failing check was retried.

The original SDK's fresh node chunks can be underaligned for C++ `FSpan`.
Successful differential cases use aligned storage; small-chunk failures stop
before underaligned construction. A separate rebuilt-SDK smoke decoded all
12 underaligned output nodes through raw bytes, checking independent storage,
endpoints and list termination without undefined typed C++ accesses. This is
not an original-C++ equivalence claim for those accesses.

Observed verification: 237 Rust behavior tests and all 61 selected named
CTest targets passed (53 data-none/eight data-prototype), as did data-none
signed-bundle integrity. The rebuilt game completed 120 prototype
`Maps/Adv8Forest.unr` ticks with exit 0, 114 presented/six occluded frames,
3104 triangle and 6105 complex-surface calls, and a final object purge from
23598 to zero. Presented-frame counters are not visual acceptance.

No retail/Linux runtime, feature-gate promotion or RAM-incident reopening is
claimed. Successful fresh-chunk node-allocation mutation and pre-existing
unchecked cache callback failures are outside this checkpoint's oracle.
The full Rust port remains active.

Exact commands, immutable sources, comparisons and preserved failures:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-span-mesh-lifecycle-cutover/acceptance.json`.


## Native actor drawing and VM guard checkpoint

Rust now owns complete actor sprite, weapon, muzzle-flash, carried-flag and
bounds drawing policy, including live callback reads, nested drawing, RNG
ordering, flag restoration and intentional partial state on failure. Native
typed storage, virtual device calls and stack scopes remain checked leaves.
Rust also owns process-wide runaway/recursion counters, exact limits, live
suppression, reset and diagnostic reentry. The native guard counters and
`GInitRunaway` entry point were removed; active reset callers migrated.

Observed native SDK differential evidence on macOS arm64, with prototype
data configured:

| Comparison | Exact ordered observable lines |
|---|---:|
| Original SDK versus Rust, 43 normal actor cases | 2114 |
| Unchanged C++ actor policy versus Rust on identical repaired SDK, all 59 cases | 3445 |
| Original SDK versus Rust, guard enabled/slow disabled | 67 |
| Original SDK versus Rust, guard disabled/slow disabled | 44 |
| Original SDK versus Rust, enabled guard with startup suppression | 66 |
| Original SDK versus Rust, disabled guard with startup suppression | 44 |

Intervals include unprefixed error-history continuations, without
normalization. The original SDK aborts at injected viewport hit exceptions:
a foreign C++ exception reaches a Rust panic catcher. That failure is
retained, not a passing oracle. Checked hit-stack/device callbacks now stop
at the first error and rethrow in the native viewport guard, preserving
mutation order. Full fault/reentry comparison therefore uses the unchanged
original actor policy on the same repaired dependency SDK as Rust.

Original `DO_GUARD_SLOW=1` SDK compilation fails because `AActor::PlayAnim`
uses Stack-dependent `unguardexecSlow` without a Stack. The matching
`unguardSlow` repairs it. All 151 included SDK objects were then separately
built with slow guards enabled; the ABI-consistent native VM consumer and
startup-suppressed variant completed. No original slow-guard parity is
claimed; mixing the differently sized native `UFunction` layouts was avoided.

Observed verification: 279 affected Rust tests and all 61 selected named
CTest targets passed (53 data-none/eight data-prototype). The rebuilt game
completed 120 prototype `Maps/Adv8Forest.unr` ticks with exit 0, 119
presented/one occluded frame, 2274 triangle and 5030 complex-surface calls,
and a final object purge from 23536 to zero. Signed-bundle integrity passed
under data-none. Presented-frame counters are not visual acceptance.

Particle routing/icon behavior is covered with zero simulation density,
not active particle simulation. No retail/Linux runtime, gate promotion or
RAM-incident reopening is claimed. Game initialization and travel were pending
at this checkpoint; the subsequent checkpoint records their accepted cutovers.
The full Rust port remains active.

Exact commands, immutable source/SDK snapshots, comparisons, retained initial
fixture/build failures and unresolved conditions:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-actor-sprite-vm-guard-cutover/acceptance.json`.

## Native game startup and travel checkpoint

Rust now owns `UGameEngine::Init`, `Browse`, viewport-to-actor matching,
`NotifyLevelChange` and `FixUpLevel` policy. The replaced native bodies were
deleted. Checked native leaves retain real object construction, virtual/script
calls, file operations and stack-scoped URL/string/error lifetimes.

Observed macOS arm64 differentials use complete frozen SDK link inputs, the
actual native ABI consumer and prototype `Engine.u`. The original game object
is compiled separately under its real `Engine` package, not included inside
the `AbiTests` package. Both sides share the two native repairs below.

| Comparison | Observed result |
|---|---|
| Startup, 34 normal/error cases plus 18 callback faults | All 52 original/Rust pairs exit 0; ordered state, diagnostics, allocation counts and live counts match |
| Startup size assertion | Only source-line metadata differs: original line 164, adapter line 235 |
| Browse/matching/notification/fixup, including 34 browse cases | All 3410 ordered records match without normalization |

The initial startup allocation observer missed out-of-line libc++ deletions,
leaving stale pointers whose address reuse changed its live count. A common
test-only deletion observer repairs the instrumentation; no live counts were
normalized away. The recorded replay-failure trace has identical ordered
request kinds, sizes, tags and nullness, with no stale-pointer collisions.
Earlier mismatches and fixture failures remain recorded.

Two real native boundary defects found by these scenarios were repaired:
`LoadMap` now catches the actual TCHAR exceptions on this platform and balances
its loader state; portable package identity now recognizes both path
separators. Registered `map_load_recovery_runtime` exercises missing-map
failure followed by a real Entry load. Registered `package_path_runtime`
serializes a real level to an absolute artifact path, converts the path through
FURL and reloads its correctly named package.

Observed verification: all 63 selected CTest targets passed (53 data-none,
10 data-prototype), and 279 affected Core/Engine/Render Rust tests passed.
The actual Wgpu prototype `Maps/Adv8Forest.unr` run completed 120 ticks with
exit 0, 2273 triangle calls, 5030 complex-surface calls and a final object
purge from 23536 to zero. All 120 frames were occluded, none presented;
this run is not presented-frame or visual acceptance. Signed-bundle integrity
passed under data-none.

Successful network-driver handoff is not covered: this SDK lacks the requested
IpDrv driver. Non-null audio travel differential, retail/Linux runtime and
visual acceptance remain unverified. No gate promotion or RAM-incident claim.
The later LoadMap, active particle and actor-spawning checkpoint below records
their integrated implementations and remaining acceptance limits separately.

Exact commands, complete source/SDK snapshots, per-case comparisons,
instrumentation corrections, retained failures and unresolved conditions:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-game-startup-travel-cutover/acceptance.json`.

## Checked boundaries, active particles and actor spawning checkpoint

`ULevel::SpawnActor` and `ULevel::SpawnPlayActor` now execute their complete
portable policies in Rust. C++ retains typed native storage, constructors,
virtual/script dispatch and checked service leaves. Genuine native consumers
exercise lifecycle/script ordering, collision-hash replacement and live flag
mutation, notification replacement/reentry, actual GameInfo Login and travel
inventory remapping, reverse pre/post-accept ordering, and exact native INT
and TCHAR failures followed by recovery. The same consumer objects were
relinked against the rebuilt SDK: all 475 actor-trace lines and 329 player
trace lines are byte-identical, without output normalization. The player
consumer covers 14 independent real LoadMap/Login scenarios. The frozen
158-input SDK already contains other Rust subsystems; this is a native
method baseline, not an all-C++ engine comparison.

Game/travel, actor/pawn, ticking/physics, pathfinding, collision, skeletal
and persistent-actor callback seams have been migrated to checked status
transport and native lifetime scopes. First native exceptions are retained
until Rust returns; neither C-unwind nor foreign unwinding through Rust is
used. The native channel superclass teardown now uses qualified base
destruction rather than recursively redispatching the virtual Destroy.
Actual LoadMap network-error teardown is covered, not successful IpDrv
handoff. A separate reproduced file-persistence failure left one writer
alive before fixture cleanup. Native scoped archive ownership repairs this:
registered `config_archive_runtime` observes INT7301, one writer destroyed,
zero live writers and zero fixture reclamations before caller recovery.

LoadMap orchestration is integrated and its current genuine SDK consumer
passes all 22 scenarios, including live callback reentry and native
reader/script/render-device faults with recovery. Complete raw differential
acceptance remains unresolved: FString/URL construction allocation traces
differ against the earlier already-Rust URL baseline. All raw traces and
failed attempts are retained; no allocation padding or output masking was
introduced. The native fade-time scalar leaf and Core FTime policy remain
outside this checkpoint; a nonzero-duration fade is not yet accepted.

Active particle rendering has genuine native evidence for 47 scenarios,
934 emitted vertices, actual list-update faults and all nine measured native
allocation-fault ordinals. Current faults return their exact INT values,
and recovery preserves native arena cleanup ordering. The WORD-index
boundary emits batches of 65540 and 8 vertices with 98343 index rows.
Full raw equality is intentionally not claimed: legacy FTransform padding
and some Flags bytes were uninitialized, while current Flags are defined
as zero. Complete raw differences and the separate historical scalar
correction are retained, not normalized or restored as undefined behavior.

After the actor cutovers, the seven selected CTests passed: data-none
`actor_spawn_contracts`, plus data-prototype `package_map_runtime`,
`path_scout_runtime`, `map_load_recovery_runtime`, `package_path_runtime`,
`object_lifecycle_runtime` and `config_archive_runtime`. The complete
`cargo test --offline --no-fail-fast -p hp2-engine -- --nocapture` run passed
382 tests with zero failures. The installed Wgpu app completed the prototype
Adv8Forest smoke for 120 ticks, exit 0, with no failure markers. Its retained
frame119 was directly inspected: nonblack forest cutscene with Harry and
Ron. All 120 frames were occluded and zero presented; captured output is
not foreground presentation acceptance. Retail data, Linux runtime and
successful network-driver handoff remain unverified. No feature gate is
promoted and no result is attributed to the closed RAM incident.

Exact commands, source fingerprints, original SDKs, full raw comparisons
and retained failed attempts are under
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/`:
`actor-spawn-and-player-travel-acceptance.json`,
`ConfigDestructor/native-ownership-acceptance.json`,
`ParticleRender/native-policy-acceptance.json`, the complete
`ParticleRender/word-index-complete-raw-differential.json`,
`LoadMap/SharedInputBeforeAttempt25/`,
`LoadMap/SharedInputAfterAttempt26/`, and
`Verification/ActorGameplay/actor-player-wgpu.json`.

## Core time, procedural lifecycle and nonzero fade checkpoint

Core FTime arithmetic/conversions, the remaining procedural-texture lifecycle
and backing policy, and LoadMap fade arithmetic now execute in Rust. C++ retains
the eight-byte time representation, typed native texture/array storage,
archive and virtual services, native clocks, and checked exception/lifetime
scopes. Wet/Ice public Lock and FSpark serialization also enter Rust; their
old inline native policies have been removed.

The genuine FTime consumer was rebuilt against the current headers and linked
against the rebuilt SDK. All 1436 defined-behavior trace rows are byte-identical
to the original native implementation, including exact result bits; real
`cmp -s` returned zero and both raw files have the same SHA-256. Nine historical
undefined/nonnumeric cases have separate before/after observations, not an
equivalence claim.

The original procedural consumer exposed a pre-existing native/script layout
mismatch: UFractalTexture.UMask was 260 in C++ versus 264 in Fire.u. Explicit
base-tail padding now matches the script fields while preserving derived
class offsets. The accepted original-policy baseline was recompiled with
that layout-only correction; it is not an untouched baseline. Both original
and current consumers execute all 21913 raw rows and finish with zero failures.
Of these, 21409 rows match exactly. The 504 retained differences comprise
368 real native-clock observations, six archive words matching the actual
native clock's low four bytes, and 130 burn callbacks that now observe the
current draw phase rather than stale unpublished state. No raw output was
normalized; complete byte equality is deliberately not claimed.

All six genuine callback-fault/reentry scenarios now finish normally with
zero consumer failures. The original unchecked SDK boundary aborted on four
scenarios and timed out on two; those observations are retained. Current
callbacks publish live state before invocation, refresh arenas after success,
stop at the first native failure, and rethrow only after Rust returns.
Observed allocator identities/events match in the normal trace, but remaining
engine-global allocations are not presented as a zero-leak proof. Destruction
of an actively borrowed native object/arena, concurrent native mutation and
partial Ice-buffer overlap remain unsupported.

The genuine LoadMap consumer also exercises a positive-duration fade through
a native render device: the current run produced 512 positive-fog draws and
one clear draw, then completed. Zero/negative duration and zero fog each
produce one clear draw. A real render-device INT9671 stops after its first
positive draw with no clear; a following load recovers successfully with
496 positive draws and one clear. Clock-dependent draw counts are observations,
not byte-equality requirements. This does not close the broader LoadMap raw
allocation differential above and is not a GPU-surface fade acceptance claim.

Observed integration verification on macOS arm64:

- `cargo test --offline --no-fail-fast -p hp2-core -p hp2-engine -p hp2-fire -p hp2-ffi -- --nocapture`:
  989 passed, zero failed, zero ignored across 132 result blocks.
- Nine original selected CTests passed: `abi_widths`, `ftime_archive`,
  `actor_spawn_contracts`, `package_map_runtime`, `path_scout_runtime`,
  `map_load_recovery_runtime`, `package_path_runtime`,
  `object_lifecycle_runtime` and `config_archive_runtime`.
  New `procedural_texture_runtime` passed after correcting invalid fixture
  preconditions: abstract-class construction, Clear-before-PostLoad, and
  unloaded-package/reflection-flag assumptions. Earlier failures are retained.
- `cmake --build --preset macos-arm64 --target hp2_macos_app --parallel 2`
  installed the rebuilt signed app. Its actual data-prototype Adv8Forest
  Wgpu smoke completed 120 ticks, exit zero, with 120 captured frames,
  no failure markers and no reported process-group resource leaks. The retained
  frame119 was directly inspected: Harry and Ron in the forest cutscene.
  Native output records zero presented and 120 occluded frames; foreground
  presentation is still unverified.

No retail or Linux runtime acceptance, successful IpDrv handoff, feature-gate
promotion or claim about the closed RAM incident follows from this checkpoint.
The complete Rust port remains in progress.

Exact commands, source/raw-trace fingerprints, baseline qualifications,
retained failed attempts and current verification results are recorded in
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/time-fractal-fade-acceptance.json`.
The full procedural differential is in
`FractalLifecycle/complete-native-before-after-raw-differential.json` under
that artifact root; current game output is in
`Verification/TimeFractalGameplay/time-fractal-wgpu.json`.


## Core diagnostic seed and Unix system command checkpoint

Rust now owns diagnostic seed parsing and requested/effective seed selection
(`hp2-core/src/rng_seed.rs`), plus Unix USystem command matching, dispatch order,
literal-space relaunch argument selection and diagnostic-loop control
(`hp2-core/src/system_command.rs`). Every native seed caller and the existing
seed contract now use generated FFI entries; `HP2RngSeed.h` and its duplicate
native implementations have been deleted. `rng_seed_contracts` is registered.

Native leaves retain getenv/srand and bootstrap metadata, output/config calls,
typed UTF-8 storage, allocation, fork/exec, exit and intentional fatal diagnostic
primitives. The relaunch UTF-8 owner stays in the native Exec scope through
RequestExit and exception rethrow. Checked callbacks preserve native exception
type/value without unwinding through Rust; Rust holds neither a native-object
exclusive borrow nor a lock across callbacks.

| Exercised surface | Profile | Observed result |
|---|---|---|
| Genuine SDK command consumer against frozen pre-cutover and current libraries | data-prototype | Both exit 0; complete 56-row event traces compare byte-for-byte equal. |
| Selected native CTests | data-none (7), data-prototype (3) | 10/10 pass, including the newly registered seed contract. |
| Affected hp2-core/hp2-ffi Rust suites | data-none | 601 pass, 0 fail, 0 ignored. |
| Installed app, `HP2_RNG_SEED=2147483648` | data-prototype | Exit 1 with the exact unsigned-decimal/range diagnostic before gameplay. |
| Installed app, `HP2_RNG_SEED=2147483647` | data-prototype | Exit 0 after 4 game ticks. |
| Installed app, `HP2_RNG_SEED=0`, Adv8Forest | data-prototype | Exit 0; 120 ticks, 1/1 map passed, no failure markers or reported resource leaks. |

The SDK consumer exercises safe matching and null input, native INT and TCHAR
output faults, successful nested reentry and an inner fault caught by the outer
output callback. A real config-writer INT 7301 stops RELAUNCH before fork, with
one writer created/destroyed and none live, then successful command recovery.
Four successful relaunch cases reach actual fork/execl with an isolated argv
witness executable: slash normalization, an internal tab, legacy quote handling
and an empty argument. This verifies OS argument handoff, not an actual ucc
game-server session. The fixture temporarily restores SIGCHLD's default
disposition to collect child status, then reinstalls the engine's handler.
Its first attempt incorrectly raced the engine's child reaper; that failed
attempt and the corrected source are retained. Fatal DEBUG commands were not run.

Exact maintained test commands:

```sh
cargo test --offline --no-fail-fast -p hp2-core -p hp2-ffi -- --nocapture
ctest --preset macos-arm64 --output-on-failure -R '^(rng_seed_contracts|core_parse_integer|core_parse_scalar|core_parse_guid|core_parse_object|core_pure_ansi|command_line_load|object_lifecycle_runtime|config_archive_runtime|package_path_runtime)$'
```

I inspected the newly captured `frame_000119.png`: visible Harry/Ron forest
geometry and text, not a black frame. Renderer counters were 0 presented /
120 occluded, with 350 SetSceneNode, 234 DrawTile, 2275 DrawTriangles and
5030 DrawComplexSurface calls. This is offscreen output evidence only.
Foreground presentation still requires an active unlocked graphical session.

Exact commands, source/trace fingerprints, SDK provenance, retained failures,
bootstrap environments and current results are recorded under
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/`
in `core-seed-system-acceptance.json`, `CoreCommandAndSeedBaselineSdk/`,
`CoreSystemCommand/` and
`Verification/CoreSeedSystemGameplay/core-seed-system-wgpu.json`.
No retail, Linux or Windows runtime acceptance or feature-gate promotion is
claimed. The Windows seed caller was migrated but not built here. The broader
LoadMap differential was still open at this checkpoint; its later acceptance
and native-reference qualifications are recorded below. The full Rust port
remains open.

## Complete LoadMap native differential checkpoint

The complete Rust LoadMap policy now has a current-SDK native differential:
both implementations pass all 22 genuine serialized-world scenarios with
zero consumer failures and exactly one callback reentry. Coverage includes
missing maps, live actor-array append, hub push/pop/peer/load, saved gameplay,
native reader and script exceptions followed by successful loads, real pending
network package generation/handoff, viewport recovery and device faults.

The reference substitutes the frozen complete C++ `UGameEngine::LoadMap`
method into the current translation unit. Both use the same 158-input frozen
SDK snapshot, with implementation and consumer objects substituted.
Its sole method correction is `catch(TCHAR*)` to
`catch(const TCHAR*)`, preserving the already accepted package-reader
recovery fix rather than restoring its historical failure. This is a
source-qualified native method, not an untouched all-C++ SDK: the surrounding
SDK includes previously ported Rust subsystems.

Both executions use identical HOME/TMPDIR strings, fresh empty user/temp
directories, RNG seed 197, the same seven native serialized packages and an
identically restored initial hub save. The full 445174-row raw logs contain
445142 byte-identical rows and exactly 32 classified clock diagnostics:
14 load durations, 16 package-unload clocks, one package-save clock and one
network-close calendar timestamp. All non-clock components of those rows and
all 22 case-result rows match. No allocation, archive, actor, zone, viewport,
exception or cleanup mismatch remains. No trace normalization, row deletion,
allocation padding or clock replacement was used; whole-file equality is
not claimed.

The earlier comparison mixed an older Rust URL implementation into its
baseline: it constructed a Vec and assigned once. A separate real-SDK
consumer proves current Rust URL construction and the original pre-Rust C++
method have byte-identical complete 51-row traces, including native capacity
and allocator tags. String growth was not changed to imitate the older Rust
implementation. A first current-SDK attempt with different HOME paths is
retained but not accepted; those paths changed file queries and allocation
sizes. The accepted comparison corrects the actual inputs, not its output.

Named current-SDK CTests `object_lifecycle_runtime`, `config_archive_runtime`
and `package_path_runtime` passed in the preceding ten-test run. The same
installed game completed Adv8Forest for 120 ticks, exit 0; its actual captured
forest frame was inspected. All frames remained occluded, so this adds no
foreground acceptance. Active particle rendering retains its separate
47-scenario/nonzero-density native proof above. No retail/Linux/Windows
runtime acceptance or feature-gate promotion is claimed.

Full commands, source qualifications, SDK snapshot, input SHA-256 values,
every unmodified differing row, raw trace hashes and retained attempts:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/LoadMap/CurrentSdkNativeReference/complete-loadmap-native-current-sdk-acceptance.json`.
The URL-origin proof is retained alongside it under
`LoadMap/StringPolicyOrigin/native-url-allocation-origin-evidence.json`.

## Audio console and launcher display-mode checkpoint

Rust now owns the complete legacy `UALAudioSubsystem::Exec` dispatch and
launcher display-mode collection. Native leaves retain typed UObject writes,
audio backend calls, SDL discovery and request-vector ownership. Every new
callback contains native exceptions and returns checked status; Rust retains
neither a mutable native-object borrow nor an arena lock across callbacks.
Parsing, selection and labels introduce no Rust heap allocation.

The genuine native audio consumer initializes the real OpenAL null backend,
executes 31 cases and destroys the native subsystem. Before/after complete
79-row traces match byte-for-byte (`cmp` exit 0), including property/global
float bits, output and guard history. Native output and virtual `StopSound`
faults preserve `INT` and `const TCHAR*` payloads. Both callbacks successfully
reenter the real command path; `StopSound` observes the updated `RollOff`.
Unknown standalone `STOPSOUNDS` remains unhandled; only the existing
`ROLLOFF` branch requests a stop. This is not physical-audio acceptance.

The genuine SDK/SDL consumer passes 13 cases with complete 161-row traces
byte-identical (`cmp` exit 0): standard/configured ordering, first-pair
deduplication, existing duplicate labels and invalid entries, newly invalid
dimensions, maximum positive-i32 UTF-8 labels, real SDL dummy discovery,
owned/borrowed video lifetime, actual unsupported-driver fallback and the
first five real native vector allocations throwing `std::bad_alloc`.
The original collection body is source-qualified only to supply its existing
capability flag as the collector argument. These baselines include previously
ported Rust SDK subsystems, not an untouched all-C++ engine.

`cargo test --offline -p hp2-core -p hp2-audio -p hp2-launcher -p hp2-ffi`
passes 761 tests across 45 nonempty result blocks. The named CTests
`launcher_smoke`, `rng_seed_contracts`, `actor_audio_contracts`,
`map_load_recovery_runtime`, `object_lifecycle_runtime`, `audio_lifecycle`
and `native_launcher_contract` pass 7/7: four `data-none` and three
`data-prototype` registrations. The rebuilt installed application completes
Adv8Forest for 120 ticks with RNG seed 0, exit 0, no failure markers or
reported resource leaks. Its actual captured frame was inspected: Harry,
Ron and red-lit forest terrain. All 120 frames are occluded, with zero
presented; foreground acceptance remains blocked.

Earlier failed qualification attempts remain recorded: the public TCHAR
alias conflicted in cbindgen; consumer global-new definitions conflicted with
SDK inline operators; a consumer stack allocator outlived its stack; and
the audio consumer initially assumed two incorrect command contracts.
Both audio baselines also failed native bootstrap with a 260-character
configuration filename before reaching the probe. The accepted comparison
uses identical shorter HOME/TMPDIR strings and fresh directories; that
native long-profile-path limitation is not fixed or hidden by this cutover.

Commands, original-source/SDK qualifications, raw traces/hashes, failed
attempts and actual-game evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/AudioConsoleAndLauncherModes/audio-console-launcher-modes-acceptance.json`.
No visible launcher GUI, physical audio, retail, Linux or Windows runtime
acceptance or feature-gate promotion is claimed. The following checkpoint
completes the next scoped Unix policies; the full Rust port remains open.

## Sound destruction callback checkpoint

`hp2-engine::audio::usound_destroy` now owns the `USound::Destroy` null gate,
status propagation and ordering: it obtains `USound::Audio` once, unregisters
only with a live subsystem, stops on a contained native-leaf exception, then
invokes the qualified `UObject::Destroy` leaf. Native code retains the static
pointer lookup, virtual `UnregisterSound` dispatch and base destruction
dispatch; both leaves use `Hp2FfiCall` and the shell uses `Hp2RethrowPending`.
`hp2-engine::audio::lip_data_init` also sets `FLipSyncData`'s `-1` maximum
amplitude sentinel after C++ initializes its ABI owner pointer. `audio_contracts`
passes non-null, null and unregister-failure order contracts plus the sentinel
contract (data-none); a rebuilt signed application passes bundle integrity
(data-none) and Adv8Forest WGPU smoke for 120 ticks (data-prototype).
No live sound destruction, physical audio, retail, Linux or foreground-frame
acceptance is claimed. Evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/SoundDestroy/sound-destroy-acceptance.json`.

## Unix bootstrap text and file-cache checkpoint

`hp2-core::unix_text` owns the complete legacy bootstrap TCHAR length,
UTF-8 encode/decode and slash-replacement algorithms. Conversion writes
directly into the original exact-capacity libc allocation; malformed UTF-8
consumption and replacement remain distinct from standard lossy decoding.
Repeated slashes are retained. Native allocation, fatal diagnostics, libc
ownership and the post-fork child remain native.

`hp2-core::file_cache` owns temporary/expired pass order, snapshot traversal,
the initial purge gate and strict signed age comparisons. Each comparison
rereads the live threshold; path and file-manager lookup retain their
original callback ordering. Native phase callbacks keep the actual lexical
`DeleteTemps`/`DeleteExpired` guards and directly constructed FString/TArray
locals alive while calling the Rust traversal. The per-entry FString is
directly constructed without a copy and unwinds natively after Rust returns.
No native setjmp environment is stored beyond its stack frame, and no
exclusive Rust native-object borrow or arena lock spans a callback.

The original complete Unix TU was frozen before replacement. Both accepted
native/Rust comparisons use the same frozen SDK inputs, same frozen current
Rust archive and identical consumer object; the qualified Unix implementation
object is substituted. The codec observer forwards the original libc malloc
request and exports the genuine private helpers. Its 7,834 encode/decode
cases plus three normalization cases match all 23,509 raw trace rows,
including exact allocation requests. Both real allocation-failure modes
retain SIGABRT, the original diagnostic and requested capacities.
This is a `data-none`, pre-appInit, host UTF-32 result; UTF-16 and
unrepresentably large input overflow paths were not runtime-exercised.

The complete cache comparison runs with actual native metadata and
`data-prototype` bootstrap, forwarding real file-manager and allocator
operations over real timestamped fixture files. Fifteen scenarios match
all 1,342 raw rows: zero/negative gates, strict age equality, missing
directories, live configuration changes, logger reentry/path replacement,
manager replacement, INT/const-TCHAR faults, exact native guard history
and recovery. The traces include allocation/reallocation/free/tag order and
remaining real files. HOME/TMPDIR/fixture paths are identical, with fresh
filesystem contents per variant; no output normalization is used.

`cargo test --offline -p hp2-core -p hp2-ffi` passes 606 tests.
`ctest --preset macos-arm64 -R '^(launcher_smoke|command_line_load|package_path_runtime|object_lifecycle_runtime|config_archive_runtime|core_app_format|rng_seed_contracts|native_launcher_contract)$' --output-on-failure`
passes 8/8: four `data-none` and four `data-prototype` registrations.
The rebuilt signed application completes Adv8Forest for 120 ticks with
RNG seed 0, exit 0, no failure markers or reported resource leaks.
The actual captured Harry/Ron forest frame was inspected. Rendering records
350 scene nodes, 234 tiles, 2,274 triangle draws and 5,030 complex surfaces;
zero frames presented and all 120 occluded. Foreground acceptance remains
blocked; capture does not satisfy it.

Retained failed attempts: cbindgen did not emit dependency-only diagnostic
constants, corrected to a typed ABI enum; the first throwaway native
allocator observer dereferenced a legitimate null tag, corrected without
changing production behavior. Commands, SDK/source qualifications, raw
traces/hashes and runtime evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/UnixTextAndFileCache/unix-text-file-cache-acceptance.json`.
No retail, Linux, Windows, foreground or feature-gate promotion is claimed.
The full in-place Rust port remains open.


## SDL client, launcher session and pre-launch round checkpoint

`hp2-platform::client_policy` owns complete SDL client defaults, initialization,
configuration, normal/crash shutdown, live viewport input/repaint traversal
and joystick lifecycle. Reflection property placement and SDK object storage
remain native. Native leaves retain virtual dispatch, exact FTime subtraction,
live indexed rereads, render-device rereads, and remove-before-delete order;
destruction does not invent a joystick-close step.

`hp2-launcher::import_session` owns initialization/save choice, import start,
worker publication, acquire/join completion, cancellation and pre-destruction
close, root refresh, picker control, source selection and final result policy.
The actual native thread captures its own source/destination and creates its
checked invocation frame on that worker thread. Strings, vectors, progress
atomics, drawing and SDL resource ownership remain native. The real importer
is reused. Its native text sink now contains assignment exceptions and
rethrows only after the inner Rust importer returns; the worker's outer
checked boundary likewise rethrows only after its Rust session call returns.
Uncaught worker failures still terminate rather than inventing UI recovery.
The explicit Emscripten importer/session branch remains native.

`hp2-launcher::launch_round` owns source precedence/discovery decisions,
complete prepare/option/UI/finish order, settings/path commit, command-prefix
validation/publication and video release. Native callbacks directly construct
the original strings, LauncherPaths/State/options and TCHAR array in lexical
scopes lasting through each nested Rust call. Configured-root validation
temporaries are constructed only after successful discovery. Video release
still precedes the UI-open result check. All checked native failures stop
without speculative cleanup; first exceptions are rethrown on the same
thread after Rust returns, without an exclusive Rust native-object borrow.

Four original complete native sources and 172 SDK link inputs were frozen
before replacement. Accepted pairs use the same current frozen Rust archive,
unchanged SDK inputs and identical consumers. Client and round consumers
reuse the same compiled consumer object. The session consumer includes the
whole original/current launcher TU; original qualification only exposes three
exact GUI statement bodies as callable GUI-used helpers. Import scenarios use
the genuine importer, filesystem, identity validation and launcher store:

| Surface | Scenarios | Assertions | Raw rows per variant | Raw comparison |
|---|---:|---:|---:|---|
| SDL client, normal | 42 | 255 | 569 | byte-identical |
| SDL client, `-defaultres` | 42 | 255 | 569 | byte-identical |
| Complete pre-launch round | 32 | — | 3,477 | byte-identical |
| Import session and actual importer/store | 28 | 191 | 4,506 | byte-identical |

No output normalization, fake services or workers, or golden updates are used.
The session's deterministic barriers surround real importer work. Its input
assets are explicitly synthetic fixtures, not playable or retail verification.
SDK bootstrap uses `data-prototype`; the isolated pre-launch/store fixtures
do not establish retail acceptance. Physical joystick hardware, real modal
pickers, allocator-fatal worker exceptions and mid-file cancellation timing
were not exercised.

`cargo test --offline -p hp2-platform -p hp2-launcher -p hp2-ffi --features hp2-ffi/platform-sdl`
passes 170 tests. The exact 17-test CTest invocation in the acceptance record
passes all registrations: ten `data-none`, seven `data-prototype`. The signed
installed app completes Adv8Forest for 120 ticks, exit 0, with no failure
markers, orphaned process group, cleanup error or reported resource leak.
Its actual retained Harry/Ron forest frame was inspected. Renderer counters
record 119 presented/one occluded, 350 scene nodes, 234 tiles, 2,275 triangle
draws and 5,030 complex surfaces. A condition recheck after this new nonzero
presentation found the display active/awake but screen capture unauthorized;
this does not satisfy foreground visual acceptance.

Retained failed probe attempts concern duplicate SDK includes, incorrect
fixture expectations, existing/read-only fixture files, and an unprepared
store root/invalid store input—not production failures hidden by retries.
Every correction precedes the accepted unchanged-consumer comparison.
Commands, exact input/source hashes, qualification records, raw traces and
actual runtime evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/LauncherAndSdlClient/launcher-sdl-acceptance.json`.
No retail, Linux, Windows, Emscripten, foreground or gate promotion is claimed.
Native viewport/window/render orchestration and other native subsystems remain;
the full in-place Rust port remains open.

## SDL viewport and application main-loop checkpoint

`hp2-platform::viewport_policy` owns complete viewport lifecycle, cursor/window
creation, input polling/action execution, command dispatch, fullscreen/resize,
render-device selection and replacement, lock/unlock, and title/mouse policy.
Native leaves retain genuine SDK operations and live object/field access.
Display/version/event/title-conversion temporaries and original guard scopes
enclose nested Rust calls. Existing geometry/input/SDL helpers are reused
directly; the obsolete decision-only viewport FFI and its two mirror/wiring
tests are deleted, not retained as compatibility aliases.

`hp2-launcher::main_loop` owns begin/tick/frame-cap/end and the complete outer
loop, reporter ordering, countdown, live-state rereads, and success-only flush.
Native FTime temporaries/subtraction retain SDK numerical behavior. NaN and
nonpositive fixed deltas keep the original comparison semantics. Native
exceptions are contained below Rust and rethrown on the same thread after
Rust returns; no exclusive native borrow spans a callback. The old C++ phase
algorithms/wrappers are removed; browser phase callers use the same new seam.
Browser runtime behavior has not been verified.

Both complete original native sources and 172 real SDK link inputs were frozen
before replacement. Strict pairs use identical compiled consumers and the same
current frozen Rust archive. Viewport pairs differ only in the viewport object.
Loop pairs use the same new viewport object and differ only in the complete
original/current launcher TU. No SDK implementation or callback is mocked:

| SDK surface | Cases per variant | Assertions per variant | Raw rows per variant | Classified changed rows |
|---|---:|---:|---:|---:|
| Viewport, dummy SDL | 10 | 103 | 783 | 16 native clock rows |
| Viewport, actual Cocoa/wgpu | 12 | 132 | 985 | 22 native clock rows |
| Viewport crash, dummy SDL | 1 | 5 | 58 | 0; byte-identical |
| Viewport crash, actual Cocoa/wgpu | 1 | 8 | 68 | 0; byte-identical |
| Complete application loop | 22 | 294 | 835 | 421 clock/ASLR rows |

Raw observations remain unchanged. Every differing field is retained and
classified; all other fields, event order, native error histories and terminal
results agree. Native oracles independently require bit-exact FTime-derived
delta/rate formulas, live countdown mutation, failure-before-publication,
reentry independence, and genuine positive frame-cap sleep.

An additional 120-tick real-engine SDK pair enables seven genuine reporters.
All seven documents compare byte-identical: lifecycle, shadow admission, actor
transition, creature generation, global tick, fidelity, and actor slots. The
retained outputs contain 4,096 shadow events, 1,024 actor owners, 16,384 tick
events and three actor-slot checkpoints. Lifecycle/creature records are empty
on this map; fidelity correctly reports the missing checkpoint manifest.
This proves enabled reporter lifetimes/flush, not lifecycle/creature fixture
events, fidelity checkpoint capture, or world-collision/static-BSP gate flows.
The viewport consumer uses genuine transient SDK UObjects; its Draw path has
`GIsRunning=False`. The loop consumers and installed game separately exercise
full world startup, real ticking and drawing.

The common differential archive retains original helper symbols needed to link
the untouched old viewport. After qualification those dead exports are removed
from the final product. Final verification: 168 affected Rust tests, 19/19 named
CTest contracts (11 `data-none`, eight `data-prototype`), and the installed
signed app running Adv8Forest for 120 ticks, exit 0, with 120 captured frames,
no failure markers, orphaned process group, cleanup error or reported resource
leak. The actual final Harry/Ron forest frame was inspected. This run records
zero presented/120 occluded frames, 350 scene nodes, 234 tiles, 2,275 triangle
draws and 5,030 complex surfaces. It is not foreground visual acceptance; the
previous screen-capture authorization blocker remains.

Retained qualification findings include a wrong queue-empty fixture assertion
(SDL polling's sentinel permits callback-generated events behind it), a dummy
SDL window used with genuine wgpu initialization, and an absent copied-dylib
install-name alias. Fixtures/prerequisites were corrected before acceptance;
failed runs are retained. Genuine installed wgpu initialization succeeds or
throws, so renderer fallback-after-false/null-construction paths were not
fabricated. Physical joystick input and modal dialogs remain unexercised.
Exact commands, hashes, native baselines, all field classifications, seven
reporter comparisons, and final runtime evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/ViewportAndMainLoop/viewport-main-loop-acceptance.json`.

Runtime claims are macOS `data-prototype` only. No retail, Linux, Windows,
Emscripten, foreground acceptance or feature-gate promotion is claimed. Native
driver/launcher presentation and other native subsystems remain; the full
in-place Rust port is still open.

## Wgpu driver and desktop launcher presentation checkpoint

`hp2-wgpu::driver_policy` owns the complete native driver execution policy:
device/resolution lifecycle, live texture/cache/dirty/mip decisions, scene and
geometry submission, drawing/counters, lock/unlock, and native-text orchestration.
`hp2-launcher::presentation_policy` owns desktop UI traversal, settings and
selection decisions, fonts/save metadata/thumbnail policy, SDL event draining,
frame/result/outer-loop ordering, and destruction sequencing. Native adapters
retain genuine typed SDK calls, object/temporary storage, joined workers,
lexical guards and native exception scopes. Exceptions return through checked
status and are rethrown only after Rust returns; no exclusive native borrow
spans a callback. The replaced desktop/driver algorithms are removed, not kept
behind compatibility aliases. The browser launcher branch is unchanged.

Complete original sources and 172 real SDK link inputs were frozen before
replacement. Each strict pair shares one compiled consumer and the same frozen
current Rust archive; only the complete target native object differs. Genuine
SDK implementations, texture/geometry objects, widgets and callbacks are not
mocked:

| Genuine SDK pair | Cases per variant | Assertions per variant | Raw rows per variant | Equal owned image pairs |
|---|---:|---:|---:|---:|
| Wgpu driver, actual Cocoa/Metal | 13 | 35 | 232 | 8 |
| Full Adv8Forest world and wgpu driver | 22 | 294 | 835 | 0 |
| Launcher non-menu/error/lifecycle cases | 6 | — | 73 | 4 |
| Launcher controlled native queue, settled menus | 1 | — | 219 | 10 |
| Launcher real bundled Latin/merged CJK fonts | 1 | — | 20 | 1 |

All 232 driver rows and every accepted launcher pair compare byte-identical,
without normalization. All image pairs have exactly equal decoded RGBA pixels.
The full-world pair preserves every raw row; 420 rows contain 720 explicitly
classified differing fields: 390 native engine addresses, 294 native FTime
clock values, 30 measured intervals, and six clock-derived rate values. Event
order, assertion rows and every other field agree. Unchanged native oracles
independently require bit-exact native delta/rate arithmetic, live mutation,
reentry, failure-before-publication and actual positive frame-cap sleep.

Launcher qualification caught two missing fixture prerequisites, not silently
accepted mismatches. The standalone consumer now installs the real process
allocator exactly as `SDLLaunch.cpp::main` does, before native request/string
construction. Actual mouse traces also proved that physical desktop polling
overwrote queued pointer positions. The accepted queue profile uses public SDL
relative mode and an event filter for tagged native queue inputs; all actual
mouse observations remain in the raw trace. This is not physical/default-mode
input acceptance. The actual selected Fullscreen row, open source popup, narrow
tabs, and Latin/Chinese saved-game row were inspected in owned render targets.
The real font atlas reports two sources merged into one font.

Retained failed attempts also include cbindgen enum-name collisions, rejected
implicit native integer narrowing, and invalid auxiliary bound-font observation
outside an ImGui frame. The latter faulted in both original and current SDK
consumers; persistent atlas observation replaced it. No production allocator,
input, or font workaround was introduced; no failed run was retried unchanged.

Final production verification: 182 affected Rust tests passed, 57 existing tests
ignored; 26/26 named CTest contracts passed (13 `data-none`, 12 `data-prototype`,
one genuine GPU test without a data label). This includes
`wgpu_surface_round_trip`, `native_typography_contracts`,
`canvas_compatibility_contracts`, `procedural_texture_runtime`, and
`map_load_recovery_runtime`. The signed installed bundle integrity check passed.
The actual installed app ran Adv8Forest for 120 ticks, exit 0, with 120 captured
frames, no failure markers, reported resource leak, cleanup error or orphaned
process group. The inspected final frame contains the real forest/Harry/Ron.
Counters: 119 presented/one occluded, 350 scene nodes, 234 tiles, 2,274 triangle
draws and 5,030 complex surfaces; UObject teardown reports 23,535 to zero.
Presented counts plus owned captures still do not establish independent
foreground visual acceptance; the prior OS capture authorization blocker remains.

Exact commands, original/current identities, all raw state/classification
records, pixels, retained failures and final product evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/WgpuAndLauncherUi/wgpu-launcher-ui-acceptance.json`.
Runtime world/GPU claims are macOS `data-prototype`; launcher request/font
consumers are `data-none`. No retail, Linux, Windows, browser, physical joystick,
modal-dialog acceptance or feature-gate promotion is claimed. Other native
subsystems and the full in-place Rust port remain open.

## Software projection and debug-geometry checkpoint

Invariant: `URender::Project`, `Deproject`, `SolveQuadratic`, `DrawCylinder`,
`DrawCircle` and both `DrawBox` overloads execute Rust policy while preserving
original floating-point results, ordered aliased writes, live callback
mutations, recursive calls, native error prefixes and native typed lifetimes.
`UnSoftLn.cpp` now contains only checked native SDK access and ABI shells.
The useful value-projection API used by Rust scene visibility and the native
live-field API share one ordered implementation; the unused copied-frame FFI
record was removed.

Before replacement, all 172 current SDK link inputs and the byte-exact original
non-UTF8 source were frozen and hashed. The unchanged compiled genuine native
consumer links against the original or replacement geometry object, the same
immutable current Rust archive, and identical other SDK inputs. It uses actual
Engine package/object bootstrap, SDL Cocoa, the Metal device and native virtual
line drawing; it does not replace the SDK with a geometry mock.

| Exercised layer | Observed result |
|---|---|
| Original/current native SDK | Both exit 0; 63 cases, 75 assertions, zero failures each |
| Complete raw native observations | 9,485 rows each; byte-identical with no normalization |
| Numerical observations within that raw result | 4,096 quadratic, 2,592 projection and 1,440 deprojection rows |
| Drawing observations within that raw result | 655 line rows; 32 defined-boundary and 28 callback-result rows |
| Actual GPU captures | All 60 decoded RGBA pairs identical; 25 contain visible geometry and 35 are blank targets |
| Existing Rust renderer/FFI contracts | 127 passed, three existing ignored; all three captured software-math oracle tests passed |
| Named native CTest contracts | 26/26 passed: 13 `data-none`, 12 `data-prototype`, one unlabelled genuine GPU test |
| Signed installed bundle | Integrity/signature check passed |
| Installed Adv8Forest gameplay | Exit 0; 120 ticks/captures; no reported failures, resource leak, cleanup error or orphaned process group |
| Installed renderer/shutdown observations | 350 scene nodes, 234 tiles, 2,274 triangle calls, 5,030 complex surfaces; 23,535 UObjects reduced to zero |

Aliasing includes output/output overlap, output/frame Zoom/half-size/projection
overlap, input/output overlap and native vector-field deprojection targets.
Actual line callbacks mutate the live frame, actor, circle location, viewport
and caller-owned placement/box, recursively draw, or throw exact native INT and
TCHAR faults. The original extra cylinder closing segment, silhouette
calculation, edge ordering, conditional visibility and mixed float/double
arithmetic are retained. Existing Core math is reused.

The first SDK fixture compilation failed on three incorrect SDK member/method
signatures; correcting those fixture calls to the existing successful SDK
conventions produced the unchanged qualified consumer. The failed compilation
is retained; no production suppression or retry-to-green was used. No golden
fixture or feature gate changed.

Both strict SDK runs recorded zero presented/60 occluded frames. The latest
installed gameplay run recorded zero presented/120 occluded frames. Owned GPU
targets were decoded/inspected, including real cylinder/circle/box geometry and
the installed forest/Harry/Ron frame. These are not independent foreground
visual proof; the earlier OS capture authorization blocker remains unchanged.

Exact compile/link/run/CTest/Cargo/smoke commands, immutable hashes, all raw
observations, decoded pixel pairs, failed fixture and installed product report:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/SoftGeometryAndResidualOwnership/software-geometry-acceptance.json`.
Runtime/SDK GPU claims are macOS arm64 `data-prototype` only. No retail, Linux,
Windows, browser or gate promotion is claimed.

The refreshed ownership inventory records 162 compiled native translation
units, not 162 unported algorithms or a port percentage. Palette quantization
and base render-device line planning already forward to Rust. A translation-unit
audit alone misses inline headers: it identified native filename-codec, archive
and file-manager policy in `Core/Inc/FFileManagerUnix.h`, subsequently replaced
and qualified in the following checkpoint. The full working in-place Rust port
remains open.

## Unix filesystem and archive checkpoint

Invariant: generic and Unix file-manager policy, Unix archive read/write policy
and strict native filename-wire conversion execute in Rust while preserving
actual SDK observations, complete binary output, live virtual/progress callback
mutations and reentry, first native exceptions and typed native lifetimes.
`FFileManagerGeneric.h` and `FFileManagerUnix.h` now retain ABI shells and checked
SDK/stdio/filesystem/locale leaves, not competing native algorithms.

Three independent implementation slices were integrated after freezing both
complete original headers and all 172 current SDK inputs. One unchanged compiled
genuine Core/Engine consumer uses actual `FArchive*` and `FFileManager*`; original
and replacement adapters have distinct qualified class names so old weak SDK
inline definitions cannot accidentally satisfy replacement calls. Both link the
same immutable current production Rust archive and identical other SDK inputs.

| Exercised layer | Observed result |
|---|---|
| Original/current genuine SDK | Both exit 0; 316 cases, 133 assertions, zero failures each |
| Complete raw native observations | 1,803 rows each; actual `cmp` reports byte-identical, without normalization |
| Complete retained binary streams | All 60 original/Rust pairs byte-identical |
| Physical filesystem boundaries | Real case-sensitive APFS ambiguity, cross-device EXDEV moves, sparse files beyond `INT_MAX`, Unicode paths and symlink-target preservation passed |
| Existing Core/FFI Rust contracts | 604 passed, zero failed, three existing GPU tests ignored |
| Named native CTest contracts | 33/33 passed: 17 `data-none`, 15 `data-prototype`, one unlabelled genuine GPU test |
| Signed installed bundle | Integrity/signature check passed |
| Installed Adv8Forest gameplay | Exit 0; 120 ticks/captures; zero failure markers or reported resource leaks; clean process-group shutdown |
| Installed renderer/UObject shutdown | 350 scene nodes, 234 tiles, 2,264 triangle calls, 5,030 complex surfaces; 23,535 UObjects reduced to zero |

Qualification exercises actual libc `funopen` callbacks, partial/error I/O,
native INT/TCHAR observer faults, recursive archive seeking, source truncation
during copy, 4-KB/64-KB boundaries and live archive-error changes during native
FName serialization. Attribute-copy semantics and the generic setter's existing
no-op remain distinct. Live timestamp changes are checked against actual native
clock bounds, retained separately rather than replaced by fabricated timestamps.
The owned case-sensitive oracle volume was detached after qualification.

The initial SDK fixture allocated its bootstrap manager before `appInit` and
faulted before producing observations; it now uses the native static bootstrap
lifetime. Initial native candidate compilation exposed incompatible generated
TCHAR aliases; the FFI now uses the existing opaque native-pointer convention.
Failed fixture/compile artifacts remain retained. An ENOSPC during freezing was
resolved by the user's approval to remove only `target/debug/incremental`;
source, installed app and acceptance artifacts were preserved, with subsequent
Cargo commands using `CARGO_INCREMENTAL=0`.

The actual installed owned target shows the forest and Harry/Ron. This run
recorded zero presented/120 occluded frames, not independent foreground proof.
The earlier OS capture-authorization blocker is unchanged. No signal-interruption
archive acceptance, retail/Linux/Windows/browser run, golden update or feature
gate promotion is claimed.

Exact commands, immutable identities, complete comparisons, failed attempts,
clock observations, installed report and refreshed ownership inventory:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/UnixFileManagerAndArchive/unix-file-manager-archive-acceptance.json`.
SDK and installed gameplay claims are macOS arm64 `data-prototype` only.
The built-in download codecs identified here are qualified below. The packed
10-bit FAnimVec lead was dormant: `UnSkeletalMesh.h` unconditionally defines
`ANIM48`; that branch is not unfinished active production policy.

## Built-in download codec checkpoint

`FCodecBWT`, `FCodecRLE`, `FCodecHuffman`, `FCodecMTF` and `FCodecFull`
now execute their algorithms, stage traversal, cancellation and progress
policy in Rust. The public native classes retain their original layout,
typed archives/bit temporaries, plugin virtual calls and checked exception/
signal leaves. The old BWT globals and native algorithm bodies are removed.
Native faults are captured before returning through Rust and rethrown only
after Rust has returned; no native-owner borrow is held across callbacks.

One unchanged genuine SDK consumer, against the frozen original and the
production Rust implementation, passed **204 cases / 340 assertions each**.
All **226,919 raw observation rows** and **323 complete byte-stream pairs**
(3,193,653 original bytes) compared equal without normalization. Coverage
includes block/run boundaries, initialized stale block tails, partial
cancellation output, native checks and INT/TCHAR faults, mutable archive
buffers, live stage append, nested pipeline entry and live notification state.
Real elapsed-time messages are retained separately, not replaced by fake
clocks or normalized into deterministic observations.

Production verification passed **33/33 named native contracts** and
**604 Rust tests**; three existing GPU tests remained ignored. The rebuilt
signed bundle passed integrity checking. Installed `Adv8Forest` gameplay
completed **120 ticks / 120 owned captures**, with zero reported failure or
resource-leak markers and UObject teardown **23,535 → 0**. The actual owned
frame shows the forest and Harry/Ron. Draw counters: 350 scene nodes, 234
tiles, 2,275 triangle calls and 5,030 complex surfaces. Presentation remained
**0 presented / 120 occluded**; this is not independent foreground evidence.

Retained failed attempts include a probe allocation before native bootstrap,
an original unchecked negative stage index, and an incorrectly encoded
Huffman exhaustion fixture. The harness defects were corrected, and the
original out-of-bounds domain is explicitly excluded rather than hidden by
a runtime workaround. Full commands, immutable SDK/consumer/installed
identities, comparisons, failure history and profile limits:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/BuiltInDownloadCodecs/built-in-download-codec-acceptance.json`.

The refreshed source inventory contains **498 Rust source files**, **162
compiled native translation units** and **160 included native headers**.
Rust is **65.57% of the measured physical source volume**, not 65.57% of the
port's behavior or remaining work. The two groups then confirmed, base archive
and render-iterator policy, are qualified below. Remaining native ABI/ownership
removal has not been exhaustively sized. Exact manifests and measurement
commands are in the acceptance report's `Verification/full-rust-progress.json`.
No retail/Linux/Windows acceptance, foreground-capture authorization, golden
update or feature-gate promotion is claimed. The full Rust port remains open.

## Base archive and render iterator checkpoint

Base `FArchive` bit-count/mask, end-of-stream, close, network-version and
host-wchar policy now executes in `hp2-core::base_archive`; `URenderIterator`
state/cursor policy executes in `hp2-engine::render_iterator`. Native class
layout, virtual dispatch, typed wire/field leaves and constructor checks remain
unchanged. Native exceptions and signal guards terminate inside checked leaves
and are rethrown only after Rust returns; callbacks hold no native-owner borrow.

Genuine original/current SDK consumers passed **386 archive cases / 624
assertions each**, **675 normal iterator assertions each**, and **three
assertions for each of two isolated constructor faults**. All **5,379 complete
raw observation rows** and **290 complete binary-buffer pairs** (11,096 original
bytes) compared equal without normalization. Coverage includes live archive
flag mutation, native virtual overrides, first faults, same-owner reentry,
signed cursor boundaries, null-frame observer retention and actual class-size/
outer checks. Undefined signed cursor overflow and dormant big-endian paths
are not claimed.

The production build, **33/33 named native contracts**, signed-bundle check and
installed `Adv8Forest` **120-tick / 120-capture** smoke passed. All **128 existing
Core/Engine/FFI test targets** completed across recorded commands: **987 Rust
tests passed**, three existing GPU tests remained ignored, and Core/Engine
documentation tests passed. The initial expanded command's **300-second
timeout remains a finding**: it recorded 899 passes before the deadline.
Only unfinished targets were subsequently executed; this was not a
retry-to-green of the completed groups.

Retained harness failures include duplicate private-header inclusion and a
viewport teardown crash shared by both original/current consumers. LLDB
identified the fixture's missing native client Engine owner; a genuine
`UGameEngine` association fixes normal teardown without changing production
code or suppressing assertions. The previously unfinished pawn-reach replay
passed separately: 0.15 seconds inside the test, 2.29 seconds external wall
time. The earlier final launch banner alone did not establish a test hang.

The actual owned frame shows textured forest and Harry/Ron. The installed run
reported no failure/resource-leak markers, with 350 scene nodes, 234 tiles,
2,275 triangle calls and 5,030 complex surfaces. **0 presented / 120 occluded**
still does not establish independent foreground presentation; OS capture
authorization remains unavailable and was not bypassed.

The refreshed inventory contains **502 Rust source files**, **162 compiled
native translation units** and **160 included native headers**. Rust is
**65.58% of measured physical source volume**, not a completion percentage.
The launcher argument-forwarding/status-text group selected at this checkpoint
is qualified below. Native object/reflection ABI, typed lifetimes, VM/member
dispatch and platform leaves still prevent a full Rust-only runtime.
Full commands, original SDK identities, failure history, comparisons and exact
source manifests:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/BaseArchiveAndRenderIterator/base-archive-render-iterator-acceptance.json`
and its `Verification/full-rust-progress.json`. Acceptance is macOS arm64
`data-prototype` only; no retail/Linux/Windows, golden update, feature-gate
promotion or full-port completion is claimed.

## Launcher command policy and remaining-native ownership checkpoint

Complete argument-forwarding and SDL status-buffer selection now execute in
`hp2-launcher::command_policy`. Native leaves retain live argv reads, original
UTF8 conversion, stack TCHAR buffers, stderr and actual FString lifetimes.
Accepted empty arguments preserve the original leading/trailing/repeated-space
behavior. Status construction happens once after Rust returns; no additional
FString copy/allocation, argv snapshot, native-owner borrow or callback lock is
introduced. Checked leaves retain the first native exception/signal state and
rethrow only after Rust returns.

One unchanged original-header consumer object was linked against complete
original/current SDLLaunch source and genuine SDK inputs. Both passed **31
cases / 87 assertions**; all **272 raw rows per side** compared exactly without
normalization, including actual allocation/release order, complete string
storage, native error payload/history, allocator reentry/live argv mutation,
UTF8 and capacity boundaries, null status and error-then-reuse.

Retained fixture failures are not production fixes: the first consumer used a
nonexistent TArray capacity accessor; the initial allocation-fault sweep also
targeted zero-size realloc release after result publication. LLDB confirmed the
original SDK terminated in FString's noexcept destructor. Recoverable allocation
fault coverage targets nonzero-size allocations; the failed release-fault run
and backtrace remain recorded rather than relabelled a pass.

The production build, **33/33 named native contracts**, **108 Rust tests**
(three existing GPU ignores), documentation tests and signed-bundle check
passed. The installed `Adv8Forest` **120-tick / 120-capture** smoke passed under
`data-prototype`, with no failure/resource-leak markers. The actual owned frame
shows textured forest and Harry/Ron; **0 presented / 120 occluded** still does
not establish independent foreground presentation. Unchanged OS capture
authorization was not retried or bypassed.

At the launcher-checkpoint baseline, four parallel ownership audits covered
all **322 shipping native source/header files** with exact manifest membership
and per-file evidence: **112 contained residual native policy**, **163 retained
only native ABI/typed leaves**, and **47 were dormant/declarative**. The
**110 uneven, potentially overlapping residual-policy groups** were not
equal-sized tasks or a completion denominator. The launcher file remains mixed:
startup/configuration, error presentation and diagnostic state machines are
still native.

The MPEG delta below reclassifies its complete parser body from active native
policy to a native ABI/typed shell. The complete baseline remains in
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RemainingNativeOwnershipAudit`;
the current delta and physical-volume measurement are in the MPEG acceptance
directory. No retail/Linux/Windows/browser acceptance, golden update,
feature-gate promotion, calendar ETA or full-port completion is claimed.

## MPEG header parser checkpoint

`Engine/MPeg/scompmpg.cpp` now retains only the platform-specific four-byte
header fetch, ABI layout boundary, and the original native fatal leaf.
`hp2-engine::mpeg_header` owns the complete defined-input header parser and
layer-II frame-size policy. The Rust layout is asserted as the original
24-byte `MPEGAUDIOHDR`, and writes occur in the original order so rejected
headers retain the same partial record mutations.

The frozen original and current original-header consumers each passed **11
cases / 31 assertions**. Their complete 24-byte records and textual traces
compare exactly without normalization, covering version 1/2 layer I/II/III,
bad sync/layer/bitrate/sample-rate/free-bitrate headers, and the legacy
12-bit sync mask that rejects the otherwise-dead v2.5 branch. A deliberately
fatal layer-III `GetMpegFrameSize` input exited by SIGABRT in both consumers
through the retained native assertion leaf. That pair uses an aborting link
stub only: it proves route equivalence, not Core's actual fatal presentation.
The original direct consumer link failure for the cold `appFailAssert` symbol
is retained. The original zero-sample-rate frame-size division is undefined
and was not executed.

`cargo test -p hp2-engine -p hp2-ffi --features renderer-wgpu` passed **416
tests** with **3 existing ignores**. The signed installed app also passed a
WGPU `Adv8Forest` **120-tick** smoke under `data-prototype`: one map passed,
no failure markers or resource leaks. It proves startup/gameplay integration,
not that this map exercises MPEG decoding or TbFile callbacks.

A full macOS CTest attempt completed with **8 failures of 136 tests** and is
not counted as acceptance: `importer_parity`, `command_line_load`,
`paths_bootstrap_contracts`, `audio_lifecycle`, `native_launcher_contract`,
`data_identity_contracts`, `determinism_double_run`, and
`renderer_smoke_wgpu`. The recorded failures include non-directory test
`TMPDIR` paths and all-occluded renderer frames; no retry was performed.


The MPEG checkpoint's physical-line/source-volume snapshot is historical,
not a project-completion or remaining-effort metric. Its frozen-source
hashes, link limitation, differential rows, fatal-route result, build and
smoke records remain under
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/MpegHeaderParser/`.
No retail/Linux/Windows/browser acceptance, feature-gate promotion or
full-port completion is claimed.

The current targeted ownership refresh reclassified `UnParticleList.cpp`,
`HP2DataImport.cpp`, `HP2LauncherPlatform.cpp`, `ULevelSummary.h`, `UnDynBsp.h`,
`UnActor.cpp`, `UnAudio.cpp`, `UnBunch.cpp`, `UnDownload.cpp`, `AStatLog.cpp`,
`S3tcCompat.cpp`, `UnCamera.cpp`, `UnCamMgr.cpp`, and `UnMeshRn.cpp` as
Rust-owned policy with native ABI, FFI, or OS-dialog leaves. `AStatLog.cpp`
and `UnCamMgr.cpp` were already leaf-only in the original audit; those
refreshes were not new classification transitions. `UnDownload.cpp` had
also been counted in an earlier accepted baseline. The old **90 / 323**
aggregate double-counted these transitions and omitted current build inputs.

The reconciled pre-expression/speech baseline has **368 first-party native
files: 164 compiled translation units and 204 included headers**. All 164
fresh compiler dependency scans succeeded. Unique accepted deltas applied
to the original 322 audit rows yield **93 policy / 182 ABI-leaf / 47 dormant**
files; 46 newly covered files add two executable policy headers and 44
ABI/leaf files. Combined: **95 policy / 226 ABI-leaf / 47 dormant**.
The inherited classifications are not a fresh semantic review of all 322
old files; preprocessing proves membership, not behavior.

`UnRender.cpp` and `UnSprite.cpp` refinements remain mixed-file changes;
they do not remove those native translation units. No file-classification
share is claimed as a completion percentage or schedule.
The deduplicated backlog, dependency scans and count corrections are under
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/`.
Dependency-ordered milestones, four work lanes, foundation acceptance and
the native-free build gate are defined in `Docs/RUST_COMPLETION_PLAN.md`.

### Expression archive and macOS speech lifecycle cutover

`UStruct::SerializeExpr` now delegates its complete recursive bytecode walk to
Rust: versioned dispatch, operand/parameter order, label/string termination,
optional-debug speculative writes and rewind, exact Teleporter exceptions,
live cursor changes, and first native failure. The legacy conversion changes
dispatch/return values, not stored script bytes. Typed archive operators,
object/name index conversion, native diagnostics and guard scopes remain
native leaves; other reflection algorithms in `UnClass.cpp` remain native.

macOS speech start/reaping now runs in Rust with inline Rust-defined state.
Disabled calls do not reap or convert; running/replaced PIDs, fork failure,
`waitpid(WNOHANG)`, `ECHILD`, unrelated errors and `_exit(127)` retain their
original behavior. Native code retains viewport object storage and checked
`FString` conversion. The currently dormant speech entrypoint gained no caller.

Verification: the rebuilt signed app passed five named CTests
(`expression_archive_contracts`, `platform_speech_contracts`,
`class_metadata_contracts`, `native_registration`, `render_console_contracts`).
These include six new expression and four speech edge/error contracts.
A genuine SDK `UStruct::SerializeExpr` consumer passed **33 original-versus-Rust
cases**, including native exception 8173 and recovery. A real-process consumer
passed speech replacement/reaping/error cases with exec redirected to silent
`/bin/cat`; `/usr/bin/say` was never run. Installed WGPU `Adv8Forest` passed
one map / 120 ticks, zero failure markers and zero reported resource leaks,
under `data-prototype`. It captured zero frames, not visual acceptance.

The first build found four width-conversion errors only in the throwaway
archive consumer. Checked-size conversions fixed them before its successful
build; no failing test was retried unchanged. The broader earlier eight-test
failure finding remains unresolved. No Linux, retail, sanitizer, audible
speech-backend or foreground visual acceptance is claimed.

Post-batch fresh scans succeeded for all **164 shipping native translation
units**, with **205 included native headers**: **369 files**, classified as
**94 policy / 228 ABI-leaf / 47 dormant**. The new typed expression header
accounts for the added file; speech moved from mixed to leaf-only. No native
translation unit or native-removal family was closed by these policy cutovers.
Exact commands, logs, original sources, differential results and both graph
snapshots are under
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/`;
see `execution-batch-acceptance.json` and
`shipping-game-launcher-ownership-ledger.json`.

### Full-port inventory includes native runtime tools

The full-project plan also includes the importer CLI and UCC commandlets,
although neither is part of the default packaged game/launcher graph. Two
additional successful fresh compiler dependency scans cover
`Import/Src/HP2ImportMain.cpp` and `UCC/Src/UCC.cpp`. Their three tool-only
included headers are `FFeedbackContextAnsi.h`, `FOutputDeviceAnsiError.h` and
`FFileManagerPlatformHP2.h`. Full current-body review classifies four added
files as residual policy and the file-manager alias as ABI/declarative.

At the runtime-tool coverage baseline, the combined configured macOS inventory
was **166 native sources plus 208 headers = 374 unique files**, classified as
**98 policy / 229 ABI-leaf / 47 dormant**, assigned exactly once to 14 unequal
migration families. The shipping subset above remained unchanged; these five
additional files expanded coverage rather than added implementation. Tool
migration/runtime acceptance at that baseline, Linux membership and a percentage were not
claimed. Native test/oracle executables may remain separate from product
dependencies; retained first-party runtime tools may not be excluded to
claim a native-free port.

Evidence in the same `RustCompletionBaseline/` directory:
`runtime-tool-compiler-dependencies.json` and
`cli-native-removal/pre-cli-native-ownership-ledger.json` preserve that combined
baseline; the explicitly named shipping snapshot preserves the earlier subset.

### Importer CLI native-entrypoint removal

The standalone importer now runs entirely through the Rust
`crates/hp2-import/src/main.rs` entrypoint and existing `output::run_import`.
`Import/Src/HP2ImportMain.cpp` is deleted. The launcher retains its separately
accounted native Options/Progress ABI; it is not silently counted as removed.
Corrosion links the CLI directly to the pinned archive/compression targets,
not hp2-ffi/the engine. CMake installs its imported executable on macOS/Linux.

```sh
cmake --preset macos-arm64
cmake --build --preset macos-arm64 --target hp2_import
ctest --preset macos-arm64 -R '^(import_cli_contracts|importer_parity)$' --output-on-failure
```

Observed **2/2 CTests passed in 4.82 seconds**: nine executable-boundary CLI
contracts plus two existing importer/Python parity tests with profile/archive
and invalid-input subcases. An independent consumer matched all **26 captured
original-native argv cases**, including invalid UTF-8, on exact status/stdout/
stderr bytes. Original-native and Rust imports produced **427 byte-identical
files**; Rust check-only passed. `cmake --install` to the artifact prefix
succeeded, and the installed executable imported the same 427 exact files.
`otool -L` listed only system `libiconv` and `libSystem`, not engine/audio/
renderer dependencies.

All importer claims above are **data-none complete synthetic fixtures**,
not actual retail-data/gameplay, Linux or foreground visual qualification.
The changed executable now passes the previously failing `importer_parity`;
the other seven earlier full-suite failures were not rerun or resolved.

Fresh scans passed for **165/165 remaining native translation units**, plus
**208 included native headers**: **373 unique files**, classified as
**97 policy / 229 ABI-leaf / 47 dormant**. Generated Rust headers are separate.
This batch removed one native translation unit; the combined UCC/ANSI tool
family and the native-free object/VM foundation remain open.

Evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/cli-native-removal/acceptance.json`,
with original sources/binary, command logs, raw argv differential, actual and
installed import outputs, and all current compiler dependency scans.
The parent `native-ownership-ledger.json` now records the 373-file inventory.


### Rust-owned VM frame backing and direct state ABI

Invariant: accepted `ProcessEvent` calls keep a plain frame in Rust-owned stack
storage for the complete synchronous callback lifetime. Checked native
placement construction establishes the C++ object lifetime; no fabricated
vtable, heap frame, blanket zeroing or mutable Rust borrow across callbacks.
`FFrame` no longer derives from `FOutputDevice`; one temporary typed adapter
preserves all six logging overloads, configured suppression and diagnostics.
The object-construction diagnostic caller uses that explicit scoped adapter.

The existing ABI generator probes all five fields. Original native size was
**48 bytes**, with offsets 8/16/24/32/40; the plain frame is **40 bytes**, with
offsets 0/8/16/24/32. Generated Rust/C++ assertions and the native FFI boundary
enforce layout/alignment and trivial destruction. Rust diagnostic traversal
and opcode dispatch now read live frame fields directly; three metadata
getters and the separate code-slot argument are removed.

```sh
cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app
ctest --preset macos-arm64 -R '^(abi_widths|native_registration|class_metadata_contracts|vm_dispatch_contracts|vm_frame_contracts|vm_call_contracts|vm_flow_contracts|vm_object_contracts|vm_state_contracts|dxt1_codec|eaxa_decoder|data_identity_contracts|config_ini_parse_semantics|config_ini_unicode_roundtrip)$' --output-on-failure
```

Observed **14/14 CTests passed in 19.32 seconds**: 13 `data-none` tests plus
the `data-prototype` native registration test. The frozen before/after SDK
consumer matched exact status/stdout/stderr: **64 genuine reflected local
assignment/return calls**, parameter and byte-opcode canaries, Unicode parent
description, six logging overloads, suppression and critical-error recovery.
The frame fixture also checks three simultaneously live reentrant frames;
its locals backing was isolated to mirror native alloca ownership, and the
changed `vm_dispatch_contracts` passed again after that fixture correction.

The full build exposed a real missing `hp2_s3tc_compress_block` dependency in
Core-only executables. The unchanged compatibility source now compiles once
as an object propagated through `hp2_ffi`, rather than relying on Engine or
Rust codegen grouping. Pinned libSquish remains
`c763145a30512c10450954b7a2b5b3a2f9a94e00`; no upstream source changed.
The focused codec/config/identity consumers above passed.

The temporary consumer CMake hook was retired; the final normal build,
generator `--check` and changed dispatch test passed. The final installed
signed WGPU executable passed **Adv8Forest, 120 ticks, data-prototype**, exit 0,
with zero reported resource leaks/failure markers and **zero captured frames**.
This is not foreground visual, Linux, sanitizer or retail qualification.
`data_identity_contracts`, like the earlier importer parity acceptance, is a
current focused pass, not proof of the historical full-suite failure's cause.
The other six historical failing targets were not rerun here.

Current ownership membership is unchanged: **373 unique native files =
165 unique compiled sources + 208 headers**; **167/167 configured dependency
scans** passed because shared path/bootstrap sources also compile for UCC.
Classifications remain **97 policy / 229 ABI-leaf / 47 dormant**. This frame
batch removes no native files and closes no entire native-removal family.
Native locals allocation, typed construction, member-pointer invocation,
other frame producers and object/class/property ownership keep M1 open.

Evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/frame-owned-state/acceptance.json`,
with compiler layouts, frozen SDK source/results, original integration
findings, focused logs, final installed smoke and fresh compiler membership.

### Checked plain native-handler dispatch

Invariant: every opcode, named native and reflected function binding uses the
same checked plain callback; the opcode cursor is consumed before invocation,
native errors never unwind through Rust, and reentry observes live table state.
Compiler-typed C++ adapters retain receiver adjustment and native exception
capture. Rust directly dispatches the table. The opaque two-word native-method
record, memcpy conversion, Itanium-only restriction and C++ frame-dispatch
callback are removed. Class static constructors remain a distinct native
member-pointer contract; this is not native-free object ownership.

```sh
cmake --preset macos-arm64 -UCMAKE_PROJECT_INCLUDE
cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app
ctest --preset macos-arm64 -R '^(abi_widths|native_registration|core_app_format|class_metadata_contracts|vm_registry_contracts|vm_dispatch_contracts|vm_frame_contracts|vm_call_contracts|vm_flow_contracts|vm_object_contracts|vm_state_contracts|dxt1_codec|eaxa_decoder|data_identity_contracts|config_ini_parse_semantics|config_ini_unicode_roundtrip)$' --output-on-failure
python3 Build/gen_engine_abi.py --check
python3 Build/smoke_maps.py --renderer=wgpu --app=dist/macos-arm64/HarryPotter2.app/Contents/MacOS/HarryPotter2 --maps=Adv8Forest --ticks=120 --timeout=90 --output=out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/plain-native-dispatch/final-installed-prototype-smoke.json --data-root=HarryPotter2/Unreal
```

Observed **16/16 CTests passed in 27.65 seconds**: 14 `data-none` contracts and
two `data-prototype` integrations. The unchanged original-versus-Rust SDK
consumer exited 0 with identical stdout/stderr: **five native cases**, including
named/indexed Abs, real Entry LevelInfo SetTimer, latent state polling and
high-native arithmetic, plus **64 script-event calls**, six logging overloads,
suppression and error recovery. Result/argument canaries and cursors are checked.
Permanent native contracts exercise duplicate replacement/restoration and
direct versus Rust-dispatched first-error recovery; Rust fixtures cover live
handler replacement during reentry. Representation/lookup identity snapshots
were removed rather than re-pinned.

Compiler-observed layout changed: native handler **16 -> 8 bytes**; lookup
record **24 -> 16**; `UFunction` **224 -> 216**, with its handler offset unchanged
at 208. The first integrated build correctly rejected cbindgen's opaque nested
`Option<handler>` pointee. The public service now exposes the initialized table
as a pointer to non-null handlers; only serialized pre-startup backing is
nullable. No unchecked null fallback or reinterpret-cast shim was added.

The final normal build and ABI generator check passed after retiring the
temporary SDK consumer hook. The installed WGPU app completed **Adv8Forest,
120 ticks, data-prototype**, exit 0, with no failure markers and **zero captured
frames**. Installed executable SHA-256:
`cec695d3e03fdbd36b9e9f9d6d768ec0cbae0eaaa9abacb06bfbadacd4670205`.
This is not foreground visual, Linux, sanitizer, retail or full-suite acceptance.
The six other historical failing targets were not rerun in this batch.

Fresh **167/167 compiler dependency scans** found **373 unique native files =
165 sources + 208 headers**; shipping game/launcher retains **369 = 164 + 205**.
Classifications remain **97 policy / 229 ABI-leaf / 47 dormant**. This cutover
removes **zero native files** and closes **zero entire native-removal families**.
Native class/object/property ownership, construction/destruction, dynamic locals
and other frame producers keep M1 open.

Evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/plain-native-dispatch/acceptance.json`,
including frozen original sources, unchanged SDK source/hash, original fixture
and integration findings, exact commands/results, final installed smoke and
fresh `final-compiler-membership.json`.

### Checked class registration and static constructors

Invariant: Rust owns the complete native class-registration sequence while
checked plain constructors preserve inherited-method suppression, live reentry,
typed defaults, first-error partial mutation and independent error recovery.

The canonical C++ adapter is specialized by the declared method, not the
registering class. All three `IMPLEMENT_CLASS` branches and both `UClass`
constructor signatures use the same checked plain callback. C++ retains typed
native method bodies, object/default storage, archive leaves and guards.

Executed on macOS arm64:

```sh
cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app hp2_class_registration_consumer
out/macos-arm64/hp2_class_registration_consumer
cmake --preset macos-arm64 -UCMAKE_PROJECT_INCLUDE
cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app
ctest --preset macos-arm64 -R '^(abi_widths|native_registration|core_app_format|class_metadata_contracts|vm_registry_contracts|vm_dispatch_contracts|vm_frame_contracts|vm_call_contracts|vm_flow_contracts|vm_object_contracts|vm_state_contracts|dxt1_codec|eaxa_decoder|data_identity_contracts|config_ini_parse_semantics|config_ini_unicode_roundtrip)$' --output-on-failure
python3 Build/gen_engine_abi.py --check
python3 Build/smoke_maps.py --renderer=wgpu --app=dist/macos-arm64/HarryPotter2.app/Contents/MacOS/HarryPotter2 --maps=Adv8Forest --ticks=120 --timeout=90 --output=out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/checked-class-construction/final-installed-prototype-smoke.json --data-root=HarryPotter2/Unreal
```

The artifact-local consumer target existed only while the deferred CMake hook
was enabled. Its frozen source SHA-256 is
`3c9f3fd96af8431527823f30a1b2e25819bfe8dbe92f6941eb9bd85ce9f5e10e`.
Original and Rust executions exited 0 with identical stdout/stderr:
**six registration cases; parent/override/failure/recovery counts each 1;
zero failures**. Cases cover parent/inherited/overriding defaults, actual
instance construction, partial constructor failure and independent recovery.
The failing class was not retried or used to construct instances.

All **16 focused CTests passed in 26.42 seconds**: 14 `data-none`, two
`data-prototype`. Rust contracts additionally cover live constructor/superclass
replacement and every fallible registration stage; native regression uses
genuine lazy class registration and engine-allocated objects.

Compiler-observed static-constructor storage changed **16 -> 8 bytes** and
`UClass` **2424 -> 2416**, with defaults/callback offsets unchanged at 2368/2392.
The final normal build and ABI generator check passed after the SDK hook was
retired. Installed WGPU **Adv8Forest** passed **120 ticks, data-prototype**,
exit 0, with no failure markers and **zero captured frames**. Executable SHA-256:
`e123c81325b52bb49654880a35f4b9772ff47cceda18b1a467355596ed1ebb44`.

Fresh **167/167 compiler dependency scans** still find **373 native files =
165 sources + 208 headers**; shipping game/launcher **369 = 164 + 205**.
Existing ownership classifications remain **97 policy / 229 ABI-leaf / 47
dormant**. No native file or complete native-removal family closes here.
M1 stays open: typed native object/class/property representation, constructor
bodies, destruction, dynamic locals and other frame producers remain.
No foreground visual, Linux, sanitizer, retail or full-suite acceptance is
claimed; the six other historical failing targets were not rerun.

Frozen originals, unchanged consumer/hash, commands/results, final installed
smoke and fresh membership:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/checked-class-construction/acceptance.json`.

### Rust-owned frame construction and call lifetime

Invariant: Rust initializes every plain `FFrame` field and owns script-call/event
frame backing for the complete synchronous continuation, including callback
reentry and first-error exits. A failed initializer never publishes an incomplete
frame or runs argument/body/copyback/destruction phases.

Both former inline native constructor algorithms now delegate to the shared
Rust initializer. C++ retains checked typed script-array access, primitive class
lookup, constructor facades, dynamic local-byte stack allocation and outer scopes.
Native `CallFunction` frame backing and event placement construction are removed;
`FStateFrame` derived initialization and other native producers remain.

Executed on macOS arm64:

```sh
cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app hp2_frame_construction_consumer
out/macos-arm64/hp2_frame_construction_consumer
cmake --build --preset macos-arm64 --target hp2_plain_dispatch_consumer
out/macos-arm64/hp2_plain_dispatch_consumer
ctest --preset macos-arm64 -R '^(abi_widths|native_registration|core_app_format|class_metadata_contracts|vm_registry_contracts|vm_dispatch_contracts|vm_frame_contracts|vm_call_contracts|vm_flow_contracts|vm_object_contracts|vm_state_contracts|dxt1_codec|eaxa_decoder|data_identity_contracts|config_ini_parse_semantics|config_ini_unicode_roundtrip)$' --output-on-failure
cmake --preset macos-arm64 -UCMAKE_PROJECT_INCLUDE
cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app
python3 Build/gen_engine_abi.py --check
python3 Build/smoke_maps.py --renderer=wgpu --app=dist/macos-arm64/HarryPotter2.app/Contents/MacOS/HarryPotter2 --maps=Adv8Forest --ticks=120 --timeout=90 --output=out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-frame-construction/final-installed-prototype-smoke.json --data-root=HarryPotter2/Unreal
```

Artifact-local SDK targets existed only while the deferred CMake hook was
enabled. Frozen constructor/call consumer SHA-256:
`f7b6907cdb264b402f16a12e50d75c859bc933d9cfc4b2c8c8378d0636404280`.
Original and Rust exit/stdout/stderr matched: **four constructors, 64 outer calls,
64 nested calls, one caught script error, one recovery, zero failures**.
The unchanged earlier reflected-event consumer also matched its preserved
native output: **64 events, five native calls, six warnings, recovered opcode
and critical errors, zero failures**.

All **16 focused CTests passed in 25.66 seconds**: 14 `data-none`, two
`data-prototype`. Rust contracts cover initializer errors before publication,
untouched output/cursors, nested call/event parent and locals lifetimes,
and early field mutation across initializer reentry and failure.
The final normal build and ABI generator check passed after hook retirement.
Installed WGPU **Adv8Forest** passed **120 ticks, data-prototype**, exit 0,
with no failure markers and **zero captured frames**. Executable SHA-256:
`8d745554440de42afe20f5523a09849fd44cfc7ba002ef76629e1e423af8bfea`.

Fresh **167/167 compiler dependency scans** still find **373 native files =
165 sources + 208 headers**; shipping game/launcher **369 = 164 + 205**.
No file or complete native-removal family closes; M1 remains open. Existing
97 policy / 229 ABI-leaf / 47 dormant classifications are historical audit
plus accepted deltas, not a fresh all-file semantic review.
No foreground visual, Linux, sanitizer, retail or full-suite acceptance is
claimed; the six other historical failing targets were not rerun.
Frozen sources, both SDK differentials, exact commands, final smoke and fresh
membership:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-frame-construction/acceptance.json`.

### Rust-owned VM local memory and event byte operations

Invariant: call/event frames and their aligned local bytes remain Rust-owned
through the complete synchronous continuation; nesting, growth and first-error
returns cannot invalidate suspended locals. Native typed property operations and
outer trace scopes keep their original ordering and error behavior.

`hp2-core::vm_locals` retains uninitialized, 16-byte-aligned buffers per thread
and nesting depth. No borrow spans callbacks. Rust owns script-call zeroing,
event parameter copy-in, trailing zeroing, direct checked handler invocation and
copyback. Metadata and frame locals are reloaded at the original live boundaries;
unused void-result offsets use wrapping arithmetic. Zero-length operations allow
null parameter pointers. Native dynamic local continuations, the locals getter,
event copy/zero/dispatch callbacks and both obsolete frame FFI exports are removed.
The existing native memory-stack pool is not reused: its native allocator pairing
and process-wide free list do not own these Rust buffers.

Executed commands (the native baseline precedes the cutover; focused CTests and
the Rust local-memory SDK run execute concurrently):

```sh
cmake --preset macos-arm64 -DCMAKE_PROJECT_INCLUDE=/Users/mike/Code/Default/hp2/out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-vm-locals/DeferredConsumer.cmake
cmake --build --preset macos-arm64 --target hp2_vm_local_memory_consumer
out/macos-arm64/hp2_vm_local_memory_consumer
cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app hp2_vm_local_memory_consumer
out/macos-arm64/hp2_vm_local_memory_consumer
ctest --preset macos-arm64 -R '^(abi_widths|native_registration|core_app_format|class_metadata_contracts|vm_registry_contracts|vm_dispatch_contracts|vm_frame_contracts|vm_locals_contracts|vm_call_contracts|vm_flow_contracts|vm_object_contracts|vm_state_contracts|dxt1_codec|eaxa_decoder|data_identity_contracts|config_ini_parse_semantics|config_ini_unicode_roundtrip)$' --output-on-failure
cmake --build --preset macos-arm64 --target hp2_frame_construction_consumer hp2_plain_dispatch_consumer
out/macos-arm64/hp2_frame_construction_consumer
out/macos-arm64/hp2_plain_dispatch_consumer
cmake --preset macos-arm64 -UCMAKE_PROJECT_INCLUDE
cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app
python3 Build/gen_engine_abi.py --check
python3 Build/smoke_maps.py --renderer=wgpu --app=dist/macos-arm64/HarryPotter2.app/Contents/MacOS/HarryPotter2 --maps=Adv8Forest --ticks=120 --timeout=90 --output=out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-vm-locals/final-installed-prototype-smoke.json --data-root=HarryPotter2/Unreal
```

Original-before/Rust-after local SDK output is identical: **320 script calls with
320 nested calls**, **256 events with 256 nested events**, **514 event handler/
trailing-zero checks**, **one typed error, one recovery, zero failures**. Requested
local sizes span zero through **262144 bytes**. Source SHA-256:
`2ea3f69633bc5bbc78d386c136d5f4ef8626b26e75b4f99ecd7ec2aeb1024d83`.
Both older unchanged constructor/reflection consumers also match native output.

The artifact-local owner probe compiled the actual `vm_locals.rs` with `rustc
--edition=2024 -O` and a counting system allocator. It observed **3,600 warmed
scopes**, **600 error returns**, checksum **190000**, and **zero allocations,
reallocations or deallocations** in that region. First-time growth, engine
callbacks and thread teardown are outside that measured region. This throwaway
probe is not a permanent allocator-count test.

All **17 focused CTests passed in 27.96 seconds**: 15 `data-none`, two
`data-prototype`. Contracts include aligned live-byte boundaries, nested panic
payload/unwinding, simultaneous thread isolation, large nested failures/recovery,
live parameter copyback metadata, destructor thresholds and void/zero-size cases.
The final installed WGPU **Adv8Forest** run passed **120 ticks, data-prototype**,
exit 0, with no failure markers and **zero captured frames**. Executable SHA-256:
`f234b31022f6e634f5531d195ccc86ef00821f59dbaf77d5ef95f27b27dcabe7`.

After hook retirement, fresh **167/167 compiler scans** find **373 native files =
165 sources + 208 headers**; shipping game/launcher **369 = 164 + 205**. No file
or complete removal family closes. Existing 97 policy / 229 ABI-leaf / 47 dormant
classifications are historical audit plus accepted deltas, not a fresh all-file
semantic review. Object/class/property ownership, other frame producers, derived
state frames and native unwind scopes keep M1 open. No foreground visual, Linux,
sanitizer, retail or full-suite acceptance is claimed; the six other historical
failing targets were not rerun. Exact commands, frozen sources, three SDK
differentials, allocation probe, final game smoke and compiler membership:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-vm-locals/acceptance.json`.

### Rust-owned persistent state frames and safe configuration filenames

Invariant: every live persistent `UObject::StateFrame` has one Rust allocation/
free pairing across initialization/reset, archive load, stack-flag removal and
object destruction. Reentry and typed archive errors retain the original committed
field prefix. Fresh latent state and padding are never read as initialized Rust
values.

The compiler-probed `FStateFrame` mirror is **72 bytes, alignment 8**, with its
40-byte `FFrame` prefix. `hp2-core::vm_frame` owns raw field initialization and
`Box<MaybeUninit<FStateFrame>>` backing; both object-instance and serialization
policy use that owner. Native allocation/deletion callbacks are removed.
The SDK constructor facade, `Describe`, typed archives and outer native lifetime
scopes remain. Obsolete commented actor state-frame allocation code was removed;
it was not a second live producer.

Executed principal commands (exact environments, earlier failures and probe
compiler commands are retained in the acceptance artifacts):

```sh
python3 Build/gen_engine_abi.py
cmake --preset macos-arm64 -DCMAKE_PROJECT_INCLUDE=/Users/mike/Code/Default/hp2/out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-state-frame/DeferredConsumer.cmake
cmake --build --preset macos-arm64 --target hp2_state_frame_consumer
out/macos-arm64/hp2_state_frame_consumer
cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app hp2_state_frame_consumer hp2_state_bootstrap_diagnostic hp2_frame_construction_consumer hp2_vm_local_memory_consumer hp2_plain_dispatch_consumer
out/macos-arm64/hp2_state_frame_consumer
out/macos-arm64/hp2_frame_construction_consumer
out/macos-arm64/hp2_vm_local_memory_consumer
out/macos-arm64/hp2_plain_dispatch_consumer
ctest --preset macos-arm64 -R '^(abi_widths|native_registration|core_app_format|class_metadata_contracts|object_lifecycle_contracts|object_gc_contracts|object_storage_contracts|object_collections_contracts|object_properties_contracts|object_construction_contracts|vm_registry_contracts|vm_dispatch_contracts|vm_frame_contracts|vm_locals_contracts|vm_call_contracts|vm_flow_contracts|vm_object_contracts|vm_state_contracts|dxt1_codec|eaxa_decoder|data_identity_contracts|config_ini_parse_semantics|config_ini_unicode_roundtrip|config_ini_typed_getters|config_ini_set_and_write|config_ini_rewrite_normalization|config_ini_cache_filenames)$' --output-on-failure
cmake --preset macos-arm64 -UCMAKE_PROJECT_INCLUDE
cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app
python3 Build/gen_engine_abi.py --check
python3 Build/smoke_maps.py --renderer=wgpu --app=dist/macos-arm64/HarryPotter2.app/Contents/MacOS/HarryPotter2 --maps=Adv8Forest --ticks=120 --timeout=90 --output=out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-state-frame/final-installed-prototype-smoke.json --data-root=HarryPotter2/Unreal
```

The corrected genuine native fixture uses concrete `UTextBuffer` owners; the
initial abstract-`UObject` fixture failure is preserved and preceded production
edits. Native-before/Rust-after SDK output is identical: **128 init/reset runs,
64 SDK stack constructors, 641 successful loads, 642 callback reentries, one typed
archive error, one partial-state check and independent recovery**, direct owner
destruction and two real GC collections, zero failures. The frozen source hash is
`320a457cad06dd6d474749b0767507ba88cf11807f81c2c99eae258c8b6dee01`.
All three older unchanged constructor/reflection/local-memory consumers also
match their native baselines. SDK bootstrap is **data-prototype**, not an isolated
no-data claim; the initial incorrect profile label remains recorded as corrected.

The allocation probe links the actual compiled Rust owner with a counting system
allocator. It measured **8,192 allocations/deallocations, zero reallocations,
589,824 bytes allocated/freed**, checksum **5001106648714967040**, without reading
fresh latent state or padding. This is isolated owner balance, not whole-engine
leak proof. Its first compile used the incompatible PATH toolchain; the retained
failure was resolved using CMake's configured Rust **1.97.1**, without changing
the toolchain pin or probe source.

One older SDK initially exited 2 without output. A bootstrap-only diagnostic
captured `Core.System Paths did not load from System.ini`: its full INI path was
256 characters, and `config_ini::cache_find_tagged` copied it without a bound into
a 256-element array. Rust now owns stable filename scratch with a stack fast path
and spill only for longer names; suffix sizing and both alias replacements are
covered. The same HOME/INI bootstrap and frozen SDK passed after the production
repair; the failure was not hidden by shortening the path. Permanent
`config_ini_cache_filenames` coverage reads distinct real files around the
boundary through explicit, extension-less, default, system and user names.

All **27 focused CTests passed in 42.03 seconds**: 25 `data-none`, two
`data-prototype`. The final normal installed WGPU app passed **Adv8Forest,
120 ticks, data-prototype**, exit 0, zero failure markers, **zero captured frames**.
Executable SHA-256:
`06830b731033446fa6bce8b079878c64dfd67d25b1abe6e86d435f7c44d57e21`.
Temporary SDK targets are retired. Fresh **167/167 compiler scans** still find
**373 native files = 165 sources + 208 headers**; shipping **369 = 164 + 205**.
No native file or full removal family closes; historical classifications remain
97 policy / 229 ABI-leaf / 47 dormant, not a new all-file body audit.
Object/class/property representation and typed lifetimes, other frame/scratch
producers and native unwind scopes keep M1 open. No foreground visual, Linux,
sanitizer, retail or full-suite acceptance is claimed; six other historical
failing targets were not rerun. Complete evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-state-frame/acceptance.json`.

### Rust-owned object base-header data and checked instance constructors

Invariant: every base-header data initializer matches the actual SDK contract,
and instance-constructor replacement/reentry/error behavior is unchanged when
Rust owns saved callback restoration and direct checked live dispatch.

The generated complete `UObject` data mirror is **72 bytes, align 8**. A frozen
genuine native compiler probe measures all nine fields, native `UClass` size
**2,416 bytes**, and its **8-byte** instance callback pointer before and after
changing the callback result from `void` to checked `INT`. No guessed offsets,
hand-written final vtable or second object registry is introduced.

Rust initializes allocated/native/static headers with raw individual field
stores, preserving native vptr/padding; static construction also preserves
`Class`. Existing `FName` checkSlow diagnostics remain in the native facade.
Class-default data follows its distinct original contract: `Index=-1`,
`_LinkerIndex=0`, null reset fields, zero flags/name, supplied `Class`, and zero
data padding. Only the vptr copy remains in its native initialization leaf.
The additional default-header SDK baseline was captured while that data leaf
was still native, after the other header/constructor cuts; it is not mislabeled
as an entirely pre-port game.

Rust snapshots the nullable checked callback before destruction and restores
it with class metadata across reentry, including a null callback. Construction
reloads the live callback after allocation and invokes it directly. The native
saved-constructor context slot and invoke continuation are removed. Checked
compiler-typed adapters preserve native exceptions through pending-error status.
The editor transaction caller is compiled and migrated but not independently
runtime-exercised. Native class representation and typed constructor/destructor
bodies remain.

Seven frozen genuine SDK consumers match their native baselines exactly:
header initialization, instance constructors, class-default headers, frame
construction, reflected events, VM local bytes and persistent state frames.
All exit 0, with unchanged source hashes and identical stdout/stderr. They
exercise real package startup, supplied allocation, replacement, typed
construction/error/recovery, metadata callback mutation/reentry/restoration and
default fields. On this `__STATIC_LINK` build startup uses `EStatic`; `ENative`
has only isolated raw-owner coverage, not a genuine native-constructor runtime
claim. The raw owner probe observes **1,024 headers**, all vptrs preserved,
768 ordinary-padding cases preserved, 256 default-padding cases zeroed and
**zero allocations/reallocations**. It uses initialized raw Rust storage, not a
fake or uninitialized C++ object, and makes no whole-game allocation claim.

Commands:

```sh
cmake --preset macos-arm64 -UCMAKE_PROJECT_INCLUDE
cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app
python3 Build/gen_engine_abi.py --check
ctest --preset macos-arm64 -R '^(abi_widths|native_registration|core_app_format|class_metadata_contracts|object_lifecycle_contracts|object_gc_contracts|object_storage_contracts|object_collections_contracts|object_properties_contracts|object_construction_contracts|vm_registry_contracts|vm_dispatch_contracts|vm_frame_contracts|vm_locals_contracts|vm_call_contracts|vm_flow_contracts|vm_object_contracts|vm_state_contracts|dxt1_codec|eaxa_decoder|data_identity_contracts|config_ini_parse_semantics|config_ini_unicode_roundtrip|config_ini_typed_getters|config_ini_set_and_write|config_ini_rewrite_normalization|config_ini_cache_filenames)$' --output-on-failure
python3 Build/smoke_maps.py --renderer=wgpu --app=dist/macos-arm64/HarryPotter2.app/Contents/MacOS/HarryPotter2 --maps=Adv8Forest --ticks=120 --timeout=90 --data-root=HarryPotter2/Unreal --output="$HP2_ARTIFACT_DIR/final-installed-prototype-smoke.json"
```

All **27 CTests pass in 37.23 seconds**: 25 `data-none`, two `data-prototype`.
Final normal installed **Adv8Forest, 120 ticks, data-prototype** passes in
19.107803 seconds, exit 0, zero failure markers and **zero captured frames**.
Installed executable SHA-256:
`5d61a06439abac0e46a3b165a8ff6a37191933be13877be86d67ef5f6d9d9cc5`.
Temporary SDK hooks are retired; **167/167** fresh compiler scans find
**373 native files = 165 sources + 208 headers**, shipping **369 = 164 + 205**,
and zero SDK compile rows. No file or complete family closes; historical
97 policy / 229 ABI-leaf / 47 dormant classifications are not a fresh all-body
audit or an effort percentage. Native field access/updates, class/property
representation, typed lifetimes, other scratch producers and unwind scopes
keep M1 open. The initial opaque-nullable-callback cbindgen build failure is
preserved; the repair spells the nullable extern-C signature inline, following
the existing service convention, without hand-editing generated headers.
No foreground visual, Linux, sanitizer, retail or full-suite acceptance is
claimed; six other historical failing targets were not rerun. Exact SDK/probe
commands, unchanged source hashes, preserved intermediate results and final proof:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-object-header-and-constructor/acceptance.json`.

### Rust-owned class-default storage and lifetime

Invariant: class-default reset/inheritance and explicit class destruction retain
native allocation/property ownership, callback-visible mutations, first errors
and independent recovery while Rust owns the sequence and raw array state.

`hp2-core::class_lifecycle` now initializes ordinary/superclass `UClass`
`ClassWithin`/`ClassCRC`, resets/inherits default bytes through the existing Rust
array owner, and sequences `NetFields`, `Dependencies`, `PackageImports`, default
property exit, default storage, default text and qualified super destruction.
The existing native allocator is reused. Service tables are copied before
callbacks; raw fields are reloaded across them. Native metadata reset/resize
continuations and duplicate default initialization are removed.

The same frozen native compiler probe preserves `UClass` **2416 / align 8**,
`FDependency` **16 / align 8** and their measured inherited/array field offsets.
Metadata services change **29 -> 30 pointer slots**; registration services stay
12. The ABI generator owns the mirrors/assertions; generated headers were not
hand-edited. The new genuine SDK freezes the additional class-lifetime boundary
after earlier Rust cuts. Native/Rust exits are 0 with identical output and
unchanged source: two inherited classes, six watched owned-allocation releases,
typed static-constructor error **8217**, retained committed defaults and an
independent successful recovery. This is not a wholly-native baseline.

The seven older frozen SDKs initially caught `CodeOffset>=0` during an existing
state-frame load: `Node` had changed to the decoded node, but `Code` still
belonged to the previous script. Rust now decodes loading offsets before code
reconstruction rather than deriving them from that stale pointer. Save-side
bounds/conversion and decoded-offset validation remain. The failure is retained;
the four-case permanent regression suite had two failing load tests and two
passing save tests before the fix, and all four pass afterward. Its fixture
uses a genuinely owned Rust state frame and contiguous real script buffers,
not fake native object images. All seven older unchanged SDK sources now match
their original exit/stdout/stderr results, including **321 saves, 321 fresh
loads, 320 existing loads and 642 callback reentries** in the state-frame SDK.

All **30 focused CTests pass in 15.16 seconds**: 28 `data-none`, two
`data-prototype`. Normal configure with `-UCMAKE_PROJECT_INCLUDE`, verification/
installed-app build and generated ABI `--check` pass. The normal installed
WGPU **Adv8Forest, 120-tick, data-prototype** run exits 0: 1/1 maps, zero failure
markers and **zero captured frames**. Final **167/167 compiler dependency scans**
find **373 native files = 165 sources + 208 headers**, shipping
**369 = 164 + 205**, with zero temporary SDK compile rows. No file or complete
native-removal family closes; 97 policy / 229 ABI-leaf / 47 dormant remains a
historical classification, not a fresh all-body audit or effort percentage.

Native class/property representation, typed array/string and property lifetime
facades, bind/text-clear/super-destroy leaves, virtual ABI, other scratch/frame
producers and unwind scopes keep M1 open. Native SDK allocator-failure,
deserialization-reset and class-destruction error/reentry coverage is not
claimed; focused Rust modeled contracts are separate evidence. No foreground
visual, retail, Linux, sanitizer or full-suite acceptance is claimed; six other
historical failing targets were not rerun. The initial SDK signed-index compile
error, disk-full link, exact scoped build-cache reclamation, real frame-load
failure and every subsequent command/result are preserved:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-class-default-lifetime/acceptance.json`.

### Rust-owned reflected metadata data construction

Invariant: registration and ordinary metadata construction preserve original
superclass/within distinctions, inherited layout, root name, typed member
lifetimes and pointer-width config registration without reading or overwriting
unowned native bytes.

Six direct raw entry points extend the existing `hp2-core::class_lifecycle`
owner. Native/static and ordinary `UField` / `UStruct` scalar initializers,
registration `UState` scalar initializers and native/static `UClass`
registration-data initializers are removed from C++. Native typed FName/FGuid
and array construction facades remain; the empty FName/FGuid defaults do not
initialize their scalar bytes. Static SuperField/ClassWithin, registration
FriendlyName, VfHash, native vptrs/padding and other members remain untouched.
The config pointer still spans ClassConfigName and ClassCRC on arm64.

The frozen genuine SDK observes **620 checks**, six bootstrap classes, two
structures, two states, two classes, six normal metadata deletions, reflected
defaults/Marker property, one static constructor and two explicit instances.
The initial fixture incorrectly expected an instance callback while initializing
class defaults. That unsupported default-count assertion was removed, not
re-pinned; explicit instances are measured relative to their pre-instance count.
The original failed source/output remain. The corrected original baseline was
captured before removing native scalar initializers. All **nine unchanged SDK
sources** match native exit/stdout/stderr. Five additional raw-Rust contracts
cover storage preservation, null/parent/self-alias inheritance, GUID self-alias,
nullable callbacks and the full config-pointer/ClassCRC overlap. They are not
fabricated native objects or evidence of manual ENative startup forms.

The actual native compiler probe matches before/after: **UField 96, UStruct 192,
UState 2264, UClass 2416 bytes; all align 8**. PROPERTY_ALIGNMENT remains 4,
parsed/compiled flags 18 and metadata/registration services 30/12 pointer slots.
All **30 focused CTests pass in 54.19 seconds**: 28 `data-none`, two
`data-prototype`. Normal SDK-free configure/build and ABI `--check` pass.
Installed WGPU **Adv8Forest, 120 ticks, data-prototype** exits 0 with zero failure
markers and **zero captured frames**. Fresh **167/167 compiler scans** find
**373 native files = 165 sources + 208 headers**, shipping **369 = 164 + 205**,
zero SDK rows and no file/family closures.

Native representation, typed lifetimes, virtual ABI, remaining field adapters,
other frame/scratch producers and unwind scopes keep M1 open. Arbitrary
pre-registration native byte preservation and manual ENative construction are
not claimed by the real SDK; raw policy contracts are separate evidence.
No foreground visual, retail, Linux, sanitizer or full-suite acceptance is
claimed; six other historical failing targets were not rerun. Exact commands,
source hashes, preserved initial oracle failure and final acceptance:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-metadata-data-construction/acceptance.json`.

### Rust-owned reflected property construction

Invariant: default/C++ property construction preserves typed Category ordering,
virtual publication/reentry, current post-publication identity, original offset,
first-error mutations and untouched subtype/link/padding bytes.

The existing `hp2-core::native_property_offsets` owner now initializes default
data, the pre-Category ArrayDim/flags prefix, post-Category Offset/CppProperty,
the property prepend and five initialized subtype fields. The checked finish
calls native virtual publication, then reads the current owner/name and appends
the original requested offset. Capacity assertion is last. Native duplicate
stores and the obsolete direct-registration export are removed; five subtype
constructor facades retain typed base/vptr lifetime only.

The genuine additional-boundary SDK passes **206 checks** with six default
descriptors, 14 C++ descriptors, nine virtual callbacks, recursive publication,
typed constructor error/recovery, matching failed-placement release and real
reflected defaults/instances/value behavior. Its first native run violated
`UEnum`'s `DECLARE_WITHIN(UStruct)` fixture precondition; the failed source/output
remain and the corrected original baseline predates production replacement.
All **ten frozen genuine SDK sources** match native exit/stdout/stderr.
Four isolated raw-Rust contracts cover byte ownership, aliases, original offset/
current key, duplicate precedence, reentry, capacity and committed error prefixes.

The actual native compiler preserves **UProperty 160, byte/bool 168, object 176,
class 184, struct 176 bytes, all align 8**; checked construction services are
24 bytes, align 8, three pointer slots. All **31 focused CTests pass in 51.01
seconds**: 29 `data-none`, two `data-prototype`. Normal SDK-free build and ABI
`--check` pass. Installed WGPU **Adv8Forest, 120 ticks, data-prototype** exits 0,
zero failure markers, **zero captured frames**. Fresh **167/167 compiler scans**
retain **373 native files = 165 sources + 208 headers**, shipping **369 =
164 + 205**, zero SDK rows and zero file/family closures.

M1 remains open: native representation, typed lifetimes, virtual ABI, remaining
field adapters, frame/scratch producers and unwind ownership remain. Real SDK
coverage excludes arbitrary pre-initialization subtype bytes, manual native
registration forms and registry-capacity exhaustion; raw contracts are separate.
No foreground visual, retail, Linux, sanitizer, full-suite or gate promotion
is claimed. Exact commands, hashes and preserved initial fixture failure:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-property-construction/acceptance.json`.

### Rust-owned reflected property layout and nested publication

Invariant: supported-host property alignment and type-specific layout retain
live callback ordering, bool packing, nested-pointer replacement, constructor
propagation, first-error committed prefixes and unrelated bytes.

The existing `hp2-core::property_link` owner now owns all eleven active type
Link data operations, alignment and fixed/dynamic-array inner publication.
Native guards, admission/count checks, typed class tests, current-owner virtuals,
archive preload/child Link and checked narrowing remain checked leaves. Rust
copies the service table before callbacks and rereads current fields afterward;
there is no property staging buffer, per-call allocation, lock or second owner.
The original configured compiler observes owner-size before alignment. Native
duplicate algorithms and publication stores are removed.

The additional genuine SDK passes **221 checks** across **11 kinds**, nine
admissions, 59 deletions, three reentries, ten typed errors/recoveries and nine
registered reflected fields with defaults, instances and independent values.
All **eleven frozen genuine SDK sources** match native exit/stdout/stderr.
The preserved first native failure was a fixture lifetime error: default C++
FString/TArray constructors cleared reflected defaults before value checks.
Using their established `E_NoInit` constructors retains every assertion; the
corrected original baseline was frozen before native replacement. The initial
Rust ABI-module import build failure and its correction remain recorded.
Fifteen isolated raw-Rust contracts separately cover untouched bytes, aliases,
reentry, live nested/flag changes, error prefixes and unsigned checked narrowing.

Actual compiler layouts: **fixed array 176, dynamic array 168, map 176 bytes,
all align 8**. Native constants: **64 bytes, align 4**; checked services:
**48 bytes, align 8, six pointer slots**. All **32 focused CTests pass in 63.73
seconds**: 30 `data-none`, two `data-prototype`. Normal SDK-free build and ABI
`--check` pass. Installed WGPU **Adv8Forest, 120 ticks, data-prototype** exits 0
in **18.730819 seconds**, zero failure markers, **zero captured frames**.
Fresh **167/167 compiler scans** retain **373 native files = 165 sources +
208 headers**, shipping **369 = 164 + 205**, zero SDK rows and zero file/family
closures. M1 remains open.

Native representation, typed lifetimes, virtual ABI, remaining field access,
frame/scratch producers and unwind ownership remain. Genuine coverage excludes
PSX2, invalid admission assertions, fatal checked-size overflow, internal
nonvirtual classification-call order, registered map/struct members and
allocator failure. No foreground visual, retail, Linux, sanitizer, full-suite
or feature-gate promotion is claimed. Exact commands, hashes and preserved
initial failures:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-property-layout/acceptance.json`.

### Rust-owned metadata fields, state hashes and checked handlers

Invariant: metadata serialization/link/bind retains narrow-field widths, live
post-callback reads, exact first-error committed prefixes, real handler behavior
and shared nested loading-layout ordering without duplicate native data owners.

The existing `hp2-core::class_metadata` now owns direct compiler-probed
serialization fields, script-array resize through the existing array owner,
state hash clear/copy/insert and checked-function-pointer installation.
`MetadataStorage` owns the shared loading-layout pointer; native property linking
reads that same Rust-owned pointer. Nine obsolete callback bodies and the C++
global are deleted. Typed archive/superclass/configuration/diagnostic/dynamic
lookup leaves remain. Services are copied before callbacks; mutable metadata and
handler-table slots are read at their original live points.

The new genuine registered SDK passes **2,373 checks**, **12 round trips**,
nine script bytes, **1,024 hash-slot checks**, four actual handler bindings,
five typed errors, one reentry, three live mutations, 12 layout observations
and 290 deletions. All **twelve frozen genuine SDK sources** match original
exit/stdout/stderr. The in-process archive exercises real virtual serializers,
not package-file name/object-index encoding.

Preserved failures: the first native fixture incorrectly assumed host alignment
for legacy property layout. The invalid predicate was deleted, not repinned;
actual reflected values/size checks remain and an explicitly requested
alignment-16 floor is now exercised. The corrected original baseline was frozen
before replacement. The first Rust-integrated build exposed an opaque
cbindgen pointer-to-Option type; the boundary now uses the actual checked
function-pointer table, reading nullable slots as `Option` inside Rust, without
a C++ cast or generated-header patch. Existing raw Rust contracts separately
cover modeled byte ownership, callback mutation, narrowing and error prefixes.

Actual native **UFunction 216 bytes, UState 2,264 bytes, both align 8**;
VfHash has **256 pointer slots at offset 216**. Actual narrow integer widths are
preserved. Checked services: **23 pointer slots**. All **32 focused CTests pass
in 57.90 seconds**: 30 `data-none`, two `data-prototype`. Commands:

```sh
ctest --preset macos-arm64 -R '^(property_layout_contracts|property_construction_contracts|object_serialization_contracts|abi_widths|native_registration|core_app_format|class_metadata_contracts|class_lifecycle_contracts|class_registration_contracts|object_lifecycle_contracts|object_gc_contracts|object_storage_contracts|object_collections_contracts|object_properties_contracts|object_construction_contracts|vm_registry_contracts|vm_dispatch_contracts|vm_frame_contracts|vm_locals_contracts|vm_call_contracts|vm_flow_contracts|vm_object_contracts|vm_state_contracts|dxt1_codec|eaxa_decoder|data_identity_contracts|config_ini_parse_semantics|config_ini_unicode_roundtrip|config_ini_typed_getters|config_ini_set_and_write|config_ini_rewrite_normalization|config_ini_cache_filenames)$' --output-on-failure
cmake --preset macos-arm64 -UCMAKE_PROJECT_INCLUDE
cmake --build --preset macos-arm64 --target hp2_verification_binaries hp2_macos_app
python3 Build/gen_engine_abi.py --check
python3 Build/smoke_maps.py --renderer=wgpu --app=dist/macos-arm64/HarryPotter2.app/Contents/MacOS/HarryPotter2 --maps=Adv8Forest --ticks=120 --timeout=90 --output="$HP2_ARTIFACT_DIR/final-installed-prototype-smoke.json" --data-root=HarryPotter2/Unreal
```

Normal SDK-free configure/build/ABI check exit 0; installed WGPU **Adv8Forest,
120 ticks, data-prototype** exits 0 in **19.046333 seconds**, zero failure
markers and **zero captured frames**. Fresh **167/167 compiler scans** retain
**373 native files = 165 sources + 208 headers**, shipping **369 = 164 + 205**,
zero SDK rows and zero native-file/family closures. M1 remains open.

Native representation, typed lifetimes, virtual ABI, remaining frame/scratch
producers and unwind ownership remain. Genuine coverage excludes class archive
versions <=61, dynamic-export mutation during nonvirtual lookup, fatal missing
exports/invalid indices/script-size mismatches, allocator failure and parallel
native-global archive loading. No foreground visual, retail, Linux, sanitizer,
full-suite or feature-gate promotion claim. Complete commands, exact source and
installed-binary hashes, native baselines, differentials and preserved failures:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-metadata-field-ownership/acceptance.json`.

### Direct checked Rust VM opcode handlers

Invariant: twelve services-free opcodes preserve exact result bits/widths,
operand consumption, cursor/result alias ordering and static native lookup while
executing without their former C++ member-handler bodies or invocation adapters.

The existing `vm_flow` owner shares one services-free implementation between
the current facade and twelve guarded Rust exports. Canonical `GNatives` and
static native lookup now hold those direct Rust pointers; the C++ member
declarations/bodies are deleted. Established exported native-handler data names
and bootstrap order remain. No second registry, member shim, vtable emulation
or extra allocation. Remaining service dispatch is explicitly exhaustive.

The added genuine SDK passes **5,000 checks**: twelve opcodes, **355 calls on
each of three routes** (frame step, canonical table, static name), 1,053 scalar
cases, 621 bytecode aliases, 12 cursor aliases with nine ordering cases, 648
unaligned operands, 576 unaligned results, 130,809 neighbor bytes, 4,260
non-cursor frame fields and one real context destruction/registry removal.
All **thirteen unchanged genuine SDK sources** match native exit/stdout/stderr.
The first original build/run passed before production replacement. The isolated
two-file patch-capture failure is recorded; exact original live hashes were
verified before scoped retained-source recovery.

A separate Rust-only executable calls the **same twelve actual checked exports**
and passes **96 checks under data-none**. It compiles the existing FFI source
against current Rust archives, with **no former C++ VM object inputs**;
`otool -L` reports only system `libiconv` and `libSystem`. This is opcode-subset
evidence, not the package/class/instance/property/script-parameter/GC slice.
Reproducer commands, exact input/source hashes and observed output:
`rust-plain-vm-handlers/rust-only-opcode-compile.json` and
`rust-plain-vm-handlers/rust-only-opcode-execution.json`.

The same 32-target focused CTest command shown above passes **32/32 in 60.49
seconds**: 30 `data-none`, two `data-prototype`. Normal SDK-hook-free
configure/build and generated ABI `--check` exit 0. Installed WGPU
**Adv8Forest, 120 ticks, data-prototype** exits 0 in **18.657306 seconds**,
zero failure markers and **zero captured frames**. Fresh **167/167 compiler
scans** retain **373 native files = 165 sources + 208 headers**, shipping
**369 = 164 + 205**, zero SDK rows and zero file/family closures.

M1 remains open: native object/class/property representation and typed lifetimes,
other handlers and frame/scratch producers, archive/virtual operations and unwind
ownership remain. Genuine opcode coverage excludes malformed/fatal inputs,
null results for value-producing opcodes, unrepresentable discriminating cursor
aliases, byte/integer-literal cursor aliases and name semantic validation.
No foreground visual, retail, Linux, sanitizer, full-suite or gate promotion
claim. Full commands, frozen baselines, all differentials, hashes and limits:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-plain-vm-handlers/acceptance.json`.

### Direct Rust scalar VM arguments and handlers

Invariant: all 53 by-value scalar natives preserve fixed-argument stepping,
live local/object/table mutation, native `P_FINISH`/debug ordering, exact
four-byte result bits and first-error state prefixes without their former C++
member bodies or invocation adapters.

The Rust owner uses the existing canonical registry directly. Every argument
starts at zero and is stepped, even when `EndFunctionParms` rewinds the cursor.
Earlier local argument slots remain live through later/debug callbacks;
computation and publication occur only after successful finishing. The engine's
debug token is `0x38`, not stock UE `0x42`. No frame/local references survive
callbacks, no per-call heap storage, no cached second registry. The four obsolete
arithmetic-only C exports are removed; reference/assignment, short-circuit and
RNG APIs remain intentionally outside this cutover.

The additional genuine SDK passes **128,210 checks**, **5,577 cases** and
**1,859 calls on each of three routes**: frame step, canonical table and static
name lookup. Observed counters include **450 typed first-error prefixes**,
2,976 local mutations, 2,235 object-reload checks, 1,854 live table replacements,
636 debug calls, 927 cursor replacements, 435 omitted-parameter cases, 1,344
nested executions and 923,544 guard bytes. All **fourteen frozen genuine SDK
sources** match original exit/stdout/stderr. The original fixture's incorrect
44 expectation for `(6*7)+(10-2)` was corrected to 50 before native freeze;
the initial failed run and exact initial source remain preserved. No production
behavior or assertion was suppressed.

The real Rust-only bytecode executable passes **488 checks under data-none**,
using the actual checked exports and canonical Rust registry: all 53 scalar
entries, twelve registered constant/flow entries, two nested expressions, four
omitted-parameter cases and two first-error cases. Its explicit inputs are Rust
archives, without former C++ VM objects; actual dynamic dependencies are only
system `libiconv` and `libSystem`. Reproducer commands and observed output:
`rust-scalar-vm-handlers/rust-only-bytecode-{compile,execution}.json`.

The 32-target focused CTest command above, extended with
`vm_scalar_contracts`, passes **33/33 in 58.12 seconds**: 31 `data-none`, two
`data-prototype`. The scalar contract includes three isolated live-mutation,
omitted-argument and first-error transition regressions. Normal SDK-hook-free
configure/build and ABI `--check` exit 0. Installed WGPU **Adv8Forest, 120 ticks,
data-prototype** exits 0 in **18.797469 seconds**, zero failure markers and
**zero captures**. Fresh 167 compiler scans all pass: 373 native files
(165 sources + 208 headers), shipping 369 (164 + 205), zero SDK rows, zero
native file/family closures.

M1 and the full Rust port remain open: native package/object/class/property
representation and typed lifetimes, other handlers, out/reference values, GC
and native unwind/virtual/archive operations remain. No malformed-input,
invalid-pointer/null-value-result, foreground visual, retail, Linux, sanitizer,
full-suite or feature-gate promotion claim. Full commands, hashes, baselines,
all differentials and unresolved conditions:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-scalar-vm-handlers/acceptance.json`.

### Direct Rust vector, rotator and extended-native VM handlers

Invariant: 40 direct checked Rust handlers preserve vector/rotator constant
component ordering, by-value argument mutation/finishing, first-error state
prefixes and extended-native incoming-context dispatch without their former
C++ member bodies or invocation adapters.

The cutover removes two constant, fourteen vector, eight rotator and sixteen
high-prefix native members plus three obsolete arithmetic/index-only C exports.
Existing canonical data-name registration remains. Fixed stack struct operands
use `MaybeUninit`, matching the empty native constructors, while float operands
start at zero. Required struct evaluators must fully initialize the slot before
successful finishing/read. No references span callbacks. Shared argument steps
reload live object/table state; the real debug token remains `0x38`. Constants
read, advance and store one component at a time. High prefixes consume the low
byte before live table lookup and forward incoming context rather than the
frame object.

All **fifteen unchanged genuine SDK consumers** match original exit/stdout/stderr.
The new consumer passes **654,382 checks**, **16,407 records**, all **4,096
extended indices through three routes**, 1,146 typed first-error cases, 174
omitted-parameter cases, 1,230 local mutations, 918 object-reload checks, 264
debug calls, 1,029 nested executions, 16,728 table replacements, 15,960 restored
slots, 1,344 cursor replacements and 8,021,280 guard bytes. The initial original
fixture failure is preserved: native struct locals were incorrectly assumed
zero-initialized. Before baseline freeze, the omission observer was corrected
to supply explicit required-struct bytes; float zero checks remain. No
production behavior or assertion was suppressed.

The real Rust-only composite executable passes **1,189 checks, data-none**:
488 prior scalar checks plus 701 new vector/extended checks. All 24 value/
constant entries, three nested expressions, four omitted-float cases, three
first-error cases and 33 high-prefix cases exercise actual production exports.
Explicit inputs are Rust archives; `otool -L` lists only system `libiconv` and
`libSystem`, without former C++ VM objects. Reproducers:
`rust-vector-extended-vm-handlers/rust-only-bytecode-{compile,execution}.json`.

Focused CTests pass **33/33 in 55.82 seconds**, then the separately selected
`vm_vector_contracts` passes **1/1 in 2.61 seconds**: 34 unique targets, 32
`data-none` and two `data-prototype`. Normal SDK-hook-free configure/build and
ABI `--check` exit 0. Installed WGPU **Adv8Forest, 120 ticks, data-prototype**
exits 0 in **18.956486 seconds**, zero failure markers and **zero captures**.
Fresh **167/167 compiler scans** retain 373 native files (165 sources + 208
headers), shipping 369 (164 + 205), zero SDK rows and zero file/family closures.

M1 and the full Rust port remain open: native package/object/class/property
representation and typed lifetimes, remaining handlers, out/reference values,
GC and virtual/archive/unwind operations remain. No required-struct omission
default, malformed-input, invalid-pointer/null-value-result, foreground visual,
retail, Linux, sanitizer, full-suite or feature-gate promotion claim. The
reflected-events NDJSON metadata-collection failure and focused-test selection
correction are retained separately. Full commands, hashes, original baseline
failure, unchanged differentials and unresolved conditions:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-vector-extended-vm-handlers/acceptance.json`.

### Single-expression Rust VM conversion handlers

Invariant: 17 direct checked Rust conversion handlers preserve initialized
operand bytes, one-expression cursor boundaries, narrow result publication and
exact first-error effects without their former C++ members or invocation adapters.

Twelve byte/int/bool/float conversions, object/name/vector/rotator truth and
vector-to-rotator conversion now own operand stepping and publication in Rust.
The canonical registry/data names remain. Monomorphized stack slots explicitly
initialize zero/null/`NAME_NONE`, including these vector/rotator locals. Existing
kernels retain bool low-bit masking, saturating float/int and byte narrowing,
NaN/signed-zero truth, rotator low-word masking and vector rotation's 65535 scale.
There is **no `P_FINISH`, terminator or debug consumption**. Exact first errors
preserve callback/cursor/object/table changes without result publication.
No slot/frame/table references span callbacks; no per-call heap allocation,
dummy services or second registry. Generic conversion/cast services remain used
by other operations.

The new genuine SDK passes **82,064 checks**, **1,830 records**, **576 root
calls on each of three routes**, 102 typed first errors, 51 omitted operands,
231 partial writers, 105 nested expressions, 435 object-reload checks, 102 live
mutations and 605,670 guard bytes. Debug/finish trap counters remain zero.
All **sixteen unchanged SDK sources** match original exit/stdout/stderr; the
new original oracle passed first-run with all six source snapshots unchanged.

The actual Rust-only composite passes **1,669 checks under data-none**:
488 scalar + 701 vector/extended + 480 conversion checks. The new block covers
all 17 entries, 22 literal cases, 17 omissions, 17 first errors, three nested
expressions and opaque Rust-token truth without claiming native object lifetime.
Explicit link inputs are Rust archives; `otool -L` lists only system `libiconv`
and `libSystem`. Its initial fixture failed: opcode 173 encoded remainder where
the independently expected expression required subtraction opcode 175.
The failed source/output remain; only that bytecode opcode was corrected.
Production and expected-result assertions were unchanged.

Focused **35/35 CTests pass in 87.92 seconds**, 33 `data-none` and two
`data-prototype`; four new isolated regressions cover omission/widths, partial
operands, no-finish boundaries, reentry mutation and first-error publication.
Normal SDK-hook-free configure/build and ABI `--check` exit 0. Installed WGPU
**Adv8Forest, 120 ticks, data-prototype** exits 0 in **10.663823 seconds**, zero
failure markers and **zero captures**. Fresh **167/167 attributable compiler
scans** retain 373 native files (165 sources + 208 headers), shipping 369
(164 + 205), zero SDK rows and zero native file/family closures. An initial
sync-helper/async metadata-collector error is preserved, not used as compiler
evidence; the installed game run was not repeated for that correction.

M1/full-port/package/object/class/property/GC ownership remain open. Other
typed/string/trig/reference/RNG handlers and virtual/archive/unwind lifetimes
remain. No malformed-input, invalid-pointer/null-value-result, foreground visual,
retail, Linux, sanitizer, full-suite or feature-gate promotion claim.
Commands, source/link hashes, original and unchanged differential outputs,
preserved failures and unresolved conditions:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-primitive-conversion-vm-handlers/acceptance.json`.

### Rust object-storage intrusive-header ownership

The existing canonical Rust registry now directly reads compiler-probed object
name/outer/index fields, publishes stable indices and maintains intrusive hash
links. Four native primitive-field callbacks, the native access struct and its
private friend are removed; the existing service table retains checked
diagnostics only. Raw field access does not inspect vptr/padding or construct a
second object model. Hashing no longer receives an unused services argument.
Slot publication, LIFO free-index mutation, stale removed links, reentry and
first-error prefixes retain original ordering.

The new genuine SDK's first original execution exits 0 under `data-prototype`:
**65,656 checks**, 8,208 created / 8,207 destroyed real objects, three diagnostics,
two reentrant destructions and three preserved unhash links. Real `UTextBuffer`
constructors, typed string destruction, collisions, growth, replacement and
error mutations are exercised. The unchanged SDK matches exactly after cutover;
all 16 earlier SDKs also match: **17/17 exact differentials**. An initial fixture
build failure is retained; the established fixture-only private-access compile
settings were added before any original execution or production mutation.

An actual native-free executable imports the production storage FFI module,
operates on its canonical registry and passes **149,598 checks**:
1,669 prior VM checks plus 147,929 storage checks, including 123,288 guard bytes,
2,050 growth headers, three first-error diagnostics, two reentrant removals and
three intrusive unlinks. First compile/run exit 0. Explicit link inputs are Rust
archives; `otool -L` contains only `libiconv.2.dylib` and `libSystem.B.dylib`.
Raw opaque Rust headers are not evidence for native constructors/vtables,
package/class loading, typed destruction or GC ownership.

Initial focused CTest: **34/35 pass, exit 8, 56.94 seconds**. The sole failure is
a migrated contract fixture's stale `.index` projection. Only that access was
corrected to `.header.index`, preserving its assertion/expected value.
Only the failed `object_storage_contracts` test was rerun: **1/1 passes, exit 0,
0.89 seconds**. The combined evidence covers **35 unique passing tests**:
33 `data-none`, two `data-prototype`; the original failure is not erased.
Normal SDK-hook-free configure/build and ABI `--check` exit 0.
Installed WGPU **Adv8Forest, 120 ticks, data-prototype**, exits 0 in
**19.288873 seconds**, zero failure markers/budget violations and **zero captures**.
Fresh **167/167 compiler scans** retain 373 native files (165 sources + 208
headers), shipping 369 (164 + 205), no SDK compile rows, unchanged membership and
zero native file/family closures.

Native allocation, typed constructors/destructors, virtual hooks and checked
diagnostics remain. M1/full-port/package/class/object/property/GC ownership remain
open. No malformed-cycle, invalid-pointer, foreground visual, retail, Linux,
sanitizer, full-suite or feature-gate promotion claim.
Commands, frozen source/output/link hashes, exact differentials, initial
failures and corrected fixture results:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-object-storage-header-ownership/acceptance.json`.

### Rust metadata archive fields and compiler-probed commandlet layout

Two implementation workers ran concurrently with exclusive files: the ABI
generator owner and metadata wire owner. One integration owner generated outputs
and ran the shared build, frozen consumers, contracts and installed game only
after both completed.

Rust selects actual compiler-probed metadata field addresses and archive order,
including legacy widths, live callback mutations and first-error field prefixes.
The native primitive receives only archive, value type and address and executes
the real typed archive operator. Reference/name interception, compound GUID/
array behavior and native exception transport remain. The obsolete native part
dispatcher/enum and redundant integer service are removed; the service table is
22 pointers. Existing byte-archive contracts cover scalar mutations and partial
errors; their process-local reference tokens are not package-wire evidence.

The compiler-derived `UCommandlet` mirror covers four strings, both 16-element
string arrays and seven flag bits. It preserves opaque compiler bytes and does
not provide typed construction/destruction, fabricated vtables or a second
object/name/class model.

The actual version-79 `Core.u` class `Core.HelloWorldCommandlet` (export 718)
passes **114 checks** before and after: four script log branches, real
`CallFunction`/`FFrame` indexed-native `GetAxes` out parameters, property wire
round-trip, archive error prefix `[19,-2,-3]`, and GC slot/hash removal.
An earlier fixture's `ProcessEvent` call left the native outputs seeded;
that false-positive check and both fixture compile failures are preserved.
The corrected consumer verifies parameter cursor and unit-vector writes using
canonical four-byte object-slot references. This genuine mixed-runtime oracle
does not establish a native-free class implementation.

**18/18 unchanged genuine SDK differentials** match exit/stdout/stderr.
**35/35 focused CTests pass**, exit 0, **80.70 seconds**: 33 `data-none`, two
`data-prototype`. Initial batch build failure from `WORD` is preserved; the
native leaf was corrected to the existing `_WORD` type without changing expected
behavior. Compiler ABI generation/check and normal SDK-hook-free verification/
app builds exit 0. Installed WGPU **Adv8Forest, 120 ticks, data-prototype** exits
0 in **19.143538 seconds**, zero budget violations and **zero captures**.
Fresh **167/167 compiler scans**, no SDK rows, retain 165 native sources + 208
headers, shipping 164 + 205: zero membership changes or file/family closures.

Native archive/virtual/config/allocation and typed-lifetime leaves remain.
M1/full-port/package/class/object/property/GC ownership remain open. No
foreground visual, retail, Linux, sanitizer, full-suite or feature-gate
promotion claim. Commands, frozen sources/outputs, source/link hashes, parallel
ownership and preserved failures:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-metadata-wire-and-commandlet-abi/acceptance.json`.

### Crash-report artifact routing

The POSIX crash reporter now copies `HP2_ARTIFACT_DIR` during its first
installation into its preallocated report-path storage and writes frame
addresses as quoted JSON strings. Isolated SIGABRT and watchdog probes both
terminated by signal 6; Python parsed each artifact report, preserving
`crash.signal` and `crash.watchdog.timeout` respectively. The rebuilt signed
bundle passed `data-none` integrity checks. Its async-signal-safe report and
watchdog policy remains a native-platform ownership candidate. Evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/CrashReporter/`.

### Level summary localization checkpoint

`ULevelSummary::PostLoad` now delegates its complete native sequence to Rust:
superclass first, then `Title` and `IdealPlayerCount` localization, nonempty
checks, typed-assignment selection, and stop-on-first-native-error ordering.
The C++ leaves remain virtual dispatch, `Localize`, physical `FString`
assignment, exception transfer, and the Emscripten fallback. Three focused Rust
contracts passed; the rebuilt signed bundle passed `data-none` integrity checks
and the installed WGPU `Adv8Forest` smoke passed under `data-prototype` (one
map, 120 ticks, zero failure markers/resource leaks). The smoke does not
directly load a localized `ULevelSummary` fixture. Artifacts:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/LevelSummary/`.

### Moving brush tracker checkpoint

`UpdateMovingBrushTracker` now routes its null handling, moving-brush decision,
and first-error ordering through Rust. C++ retains the `AActor` and tracker
virtual calls plus exception transfer. Two focused Rust contracts passed; the
rebuilt signed bundle passed `data-none` integrity checks and the installed WGPU
`Adv8Forest` smoke passed under `data-prototype` (one map, 120 ticks, zero
failure markers/resource leaks). The smoke does not directly exercise a moving
brush. Artifacts:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/MovingBrushTracker/`.

### Top-owner checkpoint

`AActor::GetLevel`, `AActor::GetTopOwner`, `AActor::IsBasedOn`,
`AActor::IsInZone`, `AActor::IsBrush`, `AActor::IsStaticBrush`,
`AActor::IsMovingBrush`, `AActor::GetPlayerPawn`, `AActor::IsPlayer`,
`AActor::GetViewRotation`, `AActor::IsPendingKill`, world-radius helpers, and
the ordered, short-circuiting `AActor::IsBlockedBy` policy now execute in Rust.
The fixed `APlayerPawn` rendering overrides also execute in Rust, preserving
their false/identity/true results. C++ retains virtual ABI, metadata/property
registration, and typed class-dispatch leaves. The registered
`actor_inline_contracts` CTest target passed (1/1, `data-none`), covering
actor-field policies, brush-pointer gating, player class guards, actor-rotation
fallback, player rendering overrides, the blocked-by level branch, and a
callback trace proving a false first blocking condition skips Other
player/projectile queries. The rebuilt signed `data-none` bundle check and
120-tick `data-prototype` WGPU `Adv8Forest` smoke passed (one map, zero failure
markers/resource leaks). The focused contract does not execute live
brush/player/projectile branches or the APawn view-rotation branch; no retail
or Linux runtime was exercised.
Artifacts: `out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/TopOwner/`.

### ABI inventory parser checkpoint

`Build/abi_inventory.py` now recognizes named casts whose target uses
`decltype(member->field)` without mistaking the member-access `>` for the
cast-target closer. `abi_inventory_contract` passed (1/1, `data-none`), and a
fresh inventory completed with 8,165 ABI records at
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RemainingNativeOwnershipRefresh/abi-inventory.json`.
## TbFile callback and lifecycle checkpoint

`Engine/MPeg/TbFile.cpp` now contains only the virtual C++ ABI, callback
function-pointer storage/invocation, mapped-buffer allocation/deletion, and
the native exception boundary. `hp2-engine::tbfile` owns open/close state
transitions and callback eligibility; `hp2-ffi::engine_tbfile` owns defined
POSIX default-file results. Native callbacks and `new[]` do not cross Rust.

A current public-header consumer and the frozen native one passed **3 cases /
32 assertions** each with byte-exact traces. They cover default POSIX
open/read/negative-read/seek/position/size restoration, custom callback
arguments/results, idempotent open/refcount publication/close reset,
`MapFile` contents, flush, and all-null callback defaults. The signed bundle
check passed; the rebuilt WGPU `Adv8Forest` 120-tick `data-prototype` smoke
passed with one map and zero resource leaks. It does not prove this map opens
a `TbFile`. Artifacts:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/TbFile/`.

## Cache-ID construction checkpoint

`Core/Inc/UnCId.h` retains only overload shape and the `UObject::GetIndex()`
ABI leaf. `hp2-core::cache_id` now owns all five `MakeCacheID` formulas,
including C++'s 32-bit intermediate wrap before a frame index is added.

The frozen C++ formula oracle and Rust FFI agreed in **15/15** comparisons:
zero/null indices, ordinary object/frame indices, all-`DWORD`-maximum
wrapping, and every public overload. The full macOS shipping graph recompiled
the Core, Engine, and Render users through the new header wrappers. Bundle
integrity passed under `data-none`; the installed WGPU `Adv8Forest` 120-tick
smoke passed under `data-prototype` with one map and zero resource leaks. It
does not prove a runtime cache hit. Artifacts:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/CacheId/`.

## Network sequence arithmetic checkpoint

`Engine/Inc/UnNet.h` retains the protocol constants, `EChannelType`, and the
inline C++ compatibility shape. `hp2-engine::net_sequence` owns
`BestSignedDifference` and `MakeRelative`; `hp2-ffi::engine_net_sequence`
exports their scalar ABI. `hp2-engine::net_conn` now uses that shared owner.

The original header consumer and the rebuilt Rust-linked consumer produced
byte-exact results for **9 protocol cases / 18 observations**, covering wrap,
both half-range boundaries, and 1,024, 16,384, and 65,536 ranges. The full
macOS shipping graph recompiled all constant and channel-enum users. Bundle
integrity passed under `data-none`; the installed WGPU `Adv8Forest` 120-tick
smoke passed under `data-prototype` with one map and zero resource leaks. It
does not exercise a network session. Artifacts:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/NetworkSequence/`.

## Replay input-event wire checkpoint

`Engine/Src/UnReplayWire.cpp` now retains only the typed `FArchive` operator
shell: byte/float serialization, `IsLoading`, and the `FReplay::FInputEvent`
ABI. `hp2-engine::net_replay_wire` owns the high-bit encoding rule, action-bit
mask, and optional analog-value decision; its three scalar exports do not
allocate, call native callbacks, or transport errors.

The existing `replay_roundtrip` oracle passed against the frozen C++ fixture in
0.36 seconds, covering byte-for-byte replay emission, round-trip structure,
truncation, and trailing-garbage behavior. The rebuilt signed bundle passed
`data-none` integrity checks; the installed WGPU `Adv8Forest` 120-tick smoke
passed under `data-prototype` with one map and zero resource leaks. It does not
exercise a replay recording or playback session. Artifacts:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/ReplayWire/`.



## Download class-default checkpoint

`Engine/Src/UnDownload.cpp` retains typed `FString` assignments, archive
serialization, temporary scopes, and exception transport. `hp2-engine::net_download`
owns `UDownload`/`UChannelDownload` parameter-literal selection and the base
compression default. The scalar FFI boundary is allocation-free and has no
native callback or exception path.

The Rust suite passed 421 tests across 88 suites. The rebuilt signed application
completed the `Adv8Forest` WGPU 120-tick smoke under `data-prototype` (one map,
zero resource leaks). It does not exercise a package download. Artifacts:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/DownloadDefaults/`.

## Demo pending-level checkpoint

`Engine/Src/UnDemoPenLev.cpp` retains the Unreal constructors, virtual ABI,
typed package-array and string storage, platform file lookup, and exception
capture. `hp2-engine::demo_pending` owns demo-driver construction/result
selection, `USES` and `WELCOME` command branching, package admission, optional
third-person flagging, success publication, and tick dispatch ordering.

The focused Rust constructor contract passed (one test), including deletion of
a failed `InitConnect` driver and preservation of a connected driver. The
rebuilt signed bundle passed `data-none` integrity checks; the installed WGPU
`Adv8Forest` 120-tick smoke passed under `data-prototype` (one map, zero
resource leaks). Neither run opens a demo playback session. Artifacts:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/DemoPendingLevel/`.

## Static package bootstrap checkpoint

`Launch/Src/HP2StaticPackages.cpp` retains only native function-pointer
identities, macro-backed class-registration leaves, and exception capture.
`hp2-engine::static_packages` owns lookup-table clearing, ordered selector
installation, capacity and zero-termination checks, plus runtime/client
registration ordering.

The focused Rust contracts passed two cases: the complete 32-slot reset,
16-lookup ordering and terminator, both registration orders, and an
undersized-table overflow. The `native_registration` integration contract
also passed under `data-prototype`, directly checking the installed native
lookup identities. The rebuilt signed bundle passed `data-none` integrity
checks; the installed WGPU `Adv8Forest` 120-tick smoke passed under
`data-prototype` (one map, zero resource leaks). Artifacts:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/StaticPackages/`.

## Unregistered surfaces

These are work items, not test tiers: generic actor movement/physics,
particles, dynamic movers, frame capture, and retail product flows. A surface
belongs in the table above only once an observable contract and a registered
`hp2_add_behavior_test` exist for it. This prevents an unregistered aspiration
from being mistaken for a passing test.


## Bunch capacity checkpoint

`hp2-engine::net_bunch::out_bunch_capacity` now owns `FOutBunch`'s signed
packet-bit capacity arithmetic; C++ retains only native connection access and
the ABI-required `FBitWriter` base construction. The Rust contract covers a
normal packet, zero, and signed-wrap capacity. The final signed app passes
bundle integrity (data-none) and Adv8Forest WGPU smoke for 120 ticks
(data-prototype). Evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/BunchCapacity/bunch-capacity-acceptance.json`.

## S3TC DXT1 descriptor and traversal checkpoint

`hp2-ffi::s3tc` now owns legacy S3TC validation, pixel-mask and palette
conversion, alpha-reference/color-key handling, encode/decode block traversal,
compressed/decompressed descriptor publication, and decode pitch/linear-size
checks. `Engine/Src/S3tcCompat.cpp` is the C ABI shell plus the pinned
libSquish `CompressMasked` leaf. `dxt1_codec` covers known four- and
three-colour blocks, crop boundaries, deterministic encode, alpha/color-key
masking, malformed descriptors and undersized pitched payloads. The rebuilt
signed app passed bundle integrity (data-none) and Adv8Forest WGPU smoke for
120 ticks (data-prototype). Evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/S3tcCompat/s3tc-acceptance.json`.

## Camera viewport lifecycle checkpoint

`hp2-engine::camera_lifecycle` now owns `UViewport` construction/destruction
ordering: client registration, configured Canvas/Input construction and
publication, timestamping, audio detachment, window closure, platform resource
release, and client unregistration. It also owns the non-reflection
`URenderDevice` decompression-format default. `Engine/Src/UnCamera.cpp` retains
only reflection registration, C++ ABI shells, virtual/archive/device leaves,
platform typed destruction and exception transport.

The focused Rust contract covers publication and teardown order, native failure
before pointer publication, and render-device default dispatch. The rebuilt
signed app passed bundle integrity under `data-none`; installed WGPU
`Adv8Forest` passed one 120-tick smoke under `data-prototype`, with no failure
markers or resource leak. No Linux, retail-data, Windows resource-release, or
foreground visual run is claimed. Evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/UnCamera/viewport-lifecycle-acceptance.json`.

## Client manager lifecycle checkpoint

`hp2-engine::cam_mgr` now owns `UClient`'s non-reflection constructor and
destructor operations, default publication (`MaintainVerticalFOV`,
`NativeText`, `ShowFPS`), Engine pointer initialization, and post-super
viewport-array serialization. `Engine/Src/UnCamMgr.cpp` retains reflection
macros, superclass/archive ABI operations, typed object fields, virtual leaves,
native guards and exception transport.

`cam_mgr_lifecycle` covers successful lifecycle order and stops default
publication before `ShowFPS` after a native failure; the retained
`cam_mgr_exec` FOV/post-edit/BRIGHTNESS/flush oracle still passes. The rebuilt
signed app passed `data-none` bundle integrity and the installed WGPU
`Adv8Forest` 120-tick smoke passed under `data-prototype`. No Linux, retail
data, or foreground visual verification is claimed. Evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/UnCamMgr/client-lifecycle-acceptance.json`.

## Core static native lookup checkpoint

`core_native_lookup` owns the fixed 32-slot `FindNative` traversal: ascending
lookup order, first non-null result, complete-miss null result, and
stop-on-native-failure. `Core/Src/UnCoreNative.cpp` retains only the typed
C++ native-handler function-pointer invocation and exception transport; the
public Core header declares the shell without importing generated FFI types.

The focused contract passed first-hit, all-miss, and failure-stop cases. The
signed app passed `data-none` bundle integrity and installed WGPU `Adv8Forest`
passed 120 ticks under `data-prototype`. No Linux, retail-data, or foreground
visual verification is claimed. Evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/UnCoreNative/native-lookup-acceptance.json`.
Superseded: `UnCoreNative.cpp` is deleted. The table and both lookups are now
Rust-defined SDK symbols; see "Rust-defined FMemStack and static native lookup
checkpoint" in `Docs/RUST_COMPLETION_PLAN.md`.

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

## Parallel canonical resources and direct VM ownership checkpoint

Five paired ownership batches completed under one integration owner. Rust owns
compiler-probed linker-record constructors and FArchive scalar initialization,
canonical intrinsic names and VM lvalues, all former GMath storage, the shared
128-slot conversion ring and the published array-allocator resource. Checked
GetAxes/GetUnAxes and ANSI/Unicode literal handlers execute directly against
those owners; replaced native storage and handler bodies are removed.

Actual production Rust-only consumers exercise OS-file and borrowed-memory
archives, canonical names and complete Core.u package maps. All 332 names,
16 imports and 720 exports match the mixed SDK oracle exactly. Dynamic linkage
contains OS libraries/frameworks, not the C++ engine. Package-table loading is
not native-class resolution, object construction, property lifetime or GC.

Observed macOS arm64 verification:

- 20/20 mixed SDK outputs match frozen exit/stdout/stderr, including the genuine
  Core.HelloWorldCommandlet consumer's 114 checks and indexed-native axes calls.
- Compiler ABI generation and the normal SDK-hook-free verification/app build
  exit 0.
- 43/43 focused CTests pass in 75.63 seconds: 41 data-none and two data-prototype.
- The production Rust resource consumer passes unpublished-resource error
  ordering, actual allocator publication/release, initialized/empty literals,
  unsigned ANSI, unaligned UTF-16 surrogates/replacement, 1023-character output
  limits with full operand consumption, shared-ring wrap and archive guard/
  vptr/padding preservation. Ring-edge expectations are source/contract evidence,
  not an executed original ring oracle.
- Real OS and memory archive consumers pass wire/seek/error-prefix checks.
  The initial external-truncation expectation was corrected for stdio caching;
  that failed run is retained, not hidden by a production special case.
- Installed WGPU Adv8Forest completes 120 data-prototype ticks with exit 0 in
  19.180724 seconds, zero budget violations and zero captures.
- Fresh compiler dependencies pass 167/167 scans with no SDK rows: 165 native
  sources and 208 headers, shipping 164 and 205. No native file/family closure
  or classification promotion.

The measured authored footprint is 70.4% Rust, not a completion/effort estimate.
M1 and the full Rust port remain open. No foreground visual, retail, Linux,
sanitizer, full-suite or feature-gate promotion claim. Existing qualification
failures and unrelated earlier-rewrite tool drift remain unresolved.
Exact commands, source/link hashes, differentials, failures and limitations:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-canonical-linker-records-and-vm-lvalues/acceptance.json`.

## Parallel checked catalog and string arguments checkpoint

The sixth paired batch moves checked registration/name binding and owning string
execution into Rust under separate workers and one integration owner. The sole
145-entry catalog seeds numeric registration once and provides immutable named
token addresses. Numeric overrides survive reinitialization without changing
named binding. Compiler-probed opcodes and a dependency-aware normal-build ABI
freshness check derive from that same catalog.

All 17 string operators, EatString and the actual Nothing optional-argument
handler execute directly in Rust. Owning argument evaluation, finish/debug
ordering, Mid's omitted count, initialized result replacement, reverse destruction
and first-error precedence retain the native contract. Replaced native methods,
declarations and checked registration/name lists are removed.

Observed macOS arm64 acceptance:

- 21/21 unchanged mixed SDK consumers match frozen exit/stdout/stderr and source
  hashes. The genuine pre-cutover string oracle covers 109 cases, including
  reflected-property result aliasing and counted embedded NULs.
- A production Rust-only process matches 105 original-native literal/optional/
  discard rows exactly and passes live registration/override/error transitions.
  The four reflected-property alias cases remain mixed-runtime evidence.
- 44/44 focused CTests pass in 90.12 seconds: 42 data-none, two data-prototype.
- Recompiled production Rust-only consumers still match every Core.u name/import/
  export and pass actual allocator/string/archive-resource contracts.
- Normal SDK-hook-free verification/app builds pass. Installed WGPU Adv8Forest
  completes 120 data-prototype ticks in 19.663131 seconds, exit 0, zero failure
  markers, zero budget violations and zero captures.
- Fresh compiler scans pass 167/167 with zero SDK rows and unchanged membership:
  165 native sources + 208 headers; shipping 164 + 205. No classification
  promotion or native file/family closure.

M1/full Rust completion remain open: package maps and direct string dispatch are
not a complete native-free class/object/property/GC runtime. No foreground visual,
retail, Linux, sanitizer, full-suite or feature-gate promotion claim. The initial
missing generated-header include failure and its fix are retained in:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-checked-catalog-and-string-arguments/acceptance.json`.

## Parallel primitive string conversions and identity checkpoint

**Invariant:** four name/object identity operators and eight primitive string
conversions execute directly in Rust without changing live operand ownership,
result widths, localized boolean backing or native cursor/terminator semantics.

The sole checked catalog has 158 entries. ObjectConst now resolves through the
canonical Rust object table; the obsolete native resolver callback is removed.
The actual exported mutable GTrue/GFalse arrays and shared ANSI byte ring move
to Rust. CRT parsing/formatting reuse the real Core policy; mixed-runtime
assertion and exception leaves remain native, while Rust-only execution uses
actual OS-backed providers.

Observed macOS arm64 evidence:

- Before edits: genuine original-native consumer, 246 cases, zero failures.
- After cutover: all 22 frozen mixed SDK consumers match exact exit/stdout/
  stderr; all 22 consumer source hashes are unchanged.
- Real production Rust-only execution matches all 246 original-native records.
  Name/object literals use the actual canonical owners. Rust-owned header
  fixtures are not evidence of package-backed native-free class loading.
- The earlier 105-case Rust-only string consumer and complete Core.u maps/
  allocator/archive-resource consumers still match.
- 44/44 focused CTests pass in 136.14 seconds, covering conversion, identity,
  ANSI, canonical object literals and prior ownership contracts.
- Hook-free normal verification/app builds pass. Installed WGPU Adv8Forest
  completes 120 data-prototype ticks in 24.311598 seconds, exit 0, zero failure
  markers, zero budget violations and zero captures.
- Fresh compiler scans pass 167/167, zero SDK rows: 165 native sources +
  208 headers; shipping 164 + 205. Classifications and membership unchanged.

M1/full Rust completion remain open; no native file/family closure or foreground
visual, retail, Linux, sanitizer, full-suite or feature-gate promotion claim.
The initial mistyped target and signed-TCHAR consumer compile failure are
preserved alongside their fixes. Exact commands and evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-text-conversions-and-identity/acceptance.json`.

## Parallel name conversions and reference mutations checkpoint

**Invariant:** three name/object/string conversions and 31 scalar/vector/
rotator reference mutations execute directly in Rust. They capture the lvalue
before RHS evaluation and preserve reentry, finish/debug ordering, result
widths/aliases, cursor movement and string ownership.

The sole checked catalog has 192 entries. The 34 replaced native methods,
registrations and declarations are deleted. Name conversion uses canonical name
services published by static initialization; object paths reuse the existing
identity orchestration and ANSI scratch ring. No new runtime hook was added.

Observed macOS arm64 evidence:

- Before edits: genuine native consumer, 726 cases, zero failures.
- Unchanged replay: 696 records exact; 28 differ. Those came from 30 rows
  whose fixture vector/rotator struct had no fields. That leaves native struct
  copy results uninitialized, so it is a fixture defect, not a behavior change.
- A corrected typed-field consumer passes 726/726, but its 30 affected rows use
  expectations derived from native literal-mode rows. No pre-cutover native
  build ran it (HEAD `81f00fd` cannot compile the shared consumer body), so
  those 30 rows remain unresolved native evidence.
- Production Rust-only execution matches all 192 name-conversion and
  literal/temp mutation records. 22 earlier SDK consumers and four Rust-only
  consumers match exactly; the FFI name-provider lifecycle test passes.
- After the final hook-free rebuild and reinstall: 44/44 focused CTests pass in
  15.01 seconds; bundle checks pass; installed WGPU Adv8Forest completes 120
  data-prototype ticks in 18.57 seconds, exit 0, zero failure markers.
- Fresh compiler scans pass 167/167, zero SDK rows: 165 native sources +
  208 headers; shipping 164 + 205. Full runtime-target reselection matches the
  prior 167 rows in both directions.

Reflected-property lvalues and short-circuit local side effects remain mixed-
runtime evidence. M1/full Rust completion remain open; no retail, Linux,
sanitizer, full-suite, foreground visual or gate-promotion claim. Evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-name-conversions-and-reference-mutations/acceptance.json`.

## Geometry conversion checkpoint

Nine Core VM natives (77, 78, 88, 58, 89, 275, 276, 1030, 227) run only from
the Rust checked catalog. Observed macOS arm64, data-prototype:

- The genuine pre-cutover native consumer (126 cases + 1 native exception)
  replays byte-identically after cutover.
- 21 earlier SDK consumers and the 726-case typed mutation consumer are unchanged.
- Final binaries: 44/44 focused + 5/5 VM contract CTests pass; bundle check
  passes; installed WGPU Adv8Forest 120 ticks in 18.54 s, zero failure markers.
- Compiler scans 167/167: 165 native sources + 208 headers.

No retail, Linux, sanitizer, full-suite, foreground visual or gate-promotion
claim. Evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-geometry-conversions/acceptance.json`.

## Type-query and random checkpoint

`DynamicCast`, `MetaCast`, `ClassIsChildOf` (258), `IsA` (303), `Rand` (167),
`FRand` (195), `VRand` (252), `RotRand` (320) and `RandRange` (1033) run only
from the Rust checked catalog. Observed macOS arm64, data-prototype:

- The genuine pre-cutover native consumer (252 cases) and its `HP2_RNG_TRACE`
  output (322 draws, script-relative callsite offsets) replay byte-identically.
- Geometry, 21 SDK, typed mutation and four Rust-only consumers are unchanged.
  The Rust-only geometry consumer matches 121/121 records (`Invert` excluded).
- Final binaries: 44/44 focused, 7/7 VM contract and `rng_seed_contracts`
  CTests pass; bundle check passes; installed WGPU Adv8Forest 120 ticks in
  18.89 s, zero failure markers.
- Compiler scans 167/167: 165 native sources + 208 headers.

No retail, Linux, sanitizer, full-suite, foreground visual or gate-promotion
claim. Evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-type-queries-and-random/acceptance.json`.

## Object-service checkpoint

`New`, `GotoState`, `Enable`, `Disable`, `IsInState`, `GetStateName`, `Log`,
`Warn`, `SaveConfig`, `ResetConfig`, plus the named natives `Localize`,
`GetPropertyText`, `SetPropertyText`, `StaticSaveConfig`, `GetEnum`,
`GetLanguage` and `DynamicLoadObject`, run only from Rust. Observed macOS arm64,
data-prototype:

- The genuine pre-cutover consumer (153 cases: real `Engine.Console` states and
  probes, `Engine.Player` config, log and warning texts with script offsets)
  replays byte-identically.
- Geometry, type/random (with RNG trace), 21 SDK, typed mutation and five
  Rust-only consumers are unchanged.
- Final binaries: 44/44 focused and 9/9 VM/frame/RNG contract CTests pass;
  bundle check passes; installed WGPU Adv8Forest 120 ticks in 18.99 s, zero
  failure markers.
- Compiler scans 167/167.

No retail, Linux, sanitizer, full-suite, foreground visual or gate-promotion
claim. Evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-object-services/acceptance.json`.

## Script-VM checkpoint

The last 27 `UObject` interpreter natives run only from Rust: `LocalVariable`,
`InstanceVariable`, `DefaultVariable`, `ClassContext`, `ArrayElement`,
`DebugInfo`, `DynArrayElement`, `DynArrayCount`, `BoolVariable`,
`StructMember`, `NativeParm`, `Switch`, `Case`, `Jump`, `JumpIfNot`, `Assert`,
`GotoLabel`, `Let`, `LetBool`, `Self`, `Context`, `VirtualFunction`,
`FinalFunction`, `GlobalFunction`, `StructCmpEq`, `StructCmpNe` and `Iterator`.
`UnCorSc.cpp` has no `IMPLEMENT_FUNCTION` left. Observed macOS arm64,
data-prototype:

- The genuine pre-cutover consumer (209 records: real compiled Console/UWindow
  script plus 66 hand-assembled tokens, including guard-history unwinds) and
  its 209-draw RNG trace replay byte-identically. The 153-case object-service
  oracle also replays identically.
- Fixed before the freeze: a missing `GotoLabel` label crashed (an `FName`
  passed by value to `%s`, inherited from UE1). It now logs
  `GotoLabel (<name>): Label not found`.
- Geometry, type/random (with RNG trace), 21 SDK, typed mutation and five
  Rust-only consumers are unchanged.
- Final binaries: 44/44 focused and 9/9 VM/frame/RNG contract CTests pass;
  bundle check passes; installed WGPU Adv8Forest 120 ticks in 19.18 s, zero
  failure markers.
- Compiler scans 167/167.

No retail, Linux, sanitizer, full-suite, foreground visual or gate-promotion
claim. Evidence:
`out/macos-arm64/Testing/HP2/Manual/Artifacts/native-loadmap-particle-render-cutover/RustCompletionBaseline/rust-script-vm/acceptance.json`.
