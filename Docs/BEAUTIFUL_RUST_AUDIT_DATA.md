# Data and audio Rust audit

Status: **nine-file hand-written continuation reviewed; not runtime-qualified**. This is an internal application workspace, edition 2024, MSRV 1.99. The audit preserves existing ABI, byte, numerical, RNG, callback and teardown behavior. It does not approve a behavior change. Scope is the four named crates; selected engine call sites were read only to trace production ownership.

## Findings

### Must investigate: ice resample can alias mutable and shared slices

`crates/hp2-fire/src/lifecycle.rs:745–768` can set `IceLocal` to this texture's mip when the source is the texture itself and the glass is absent or mismatched. `source_update` explicitly permits a self-source (`:540–548`). The production `crates/hp2-engine/src/fractal_water_owner.rs:429–454` then creates a mutable slice for that mip and a separate shared slice for the source mip before calling `ice::resample_source`. If both point to the same nonempty allocation, those Rust references overlap, violating exclusive-reference requirements even though the kernel supports `IceRead::Dest` (`ice.rs:145–159,288–298`). **Rule:** BeautifulRust `rust-best-practices` §8 and `Docs/RUST_STYLE.md`, sound unsafe boundaries and read-after-write behavior. **Next step:** characterize self-source resampling and aliasing in a separate test, then select `IceRead::Dest` before constructing a shared slice for the same allocation; verify other overlapping allocations and preserve operation order. This is a source-level path, not an observed runtime failure.

### Should investigate: fire field tests use byte-aligned mirror storage

`crates/hp2-fire/src/lifecycle.rs:1216–1223,1229–1233` creates `Vec<u8>` storage and casts it to generated texture types in `Host::get`/`set` (`:282–350,354–430`). Byte storage does not guarantee the alignment or initialized valid values of those typed objects; similar casts occur in the remaining field tests. A passing test cannot establish safety. **Rule:** BeautifulRust `rust-best-practices` §8, valid aligned unsafe access. **Next step:** construct suitably aligned, valid typed fixture storage and characterize required mirror fields in a separate test change; do not change production layouts as a style fix.

### Must investigate: XA destination alignment

The following finding describes the earlier reviewed source; concurrent changes are noted under source drift below.

`crates/hp2-audio/src/file_stream.rs:278–284` casts a caller's byte destination to `*mut i16`, and `crates/hp2-codecs/src/eaxa.rs:130–140` writes through that pointer. The stream-create safety contract at `file_stream.rs:552–555` requires writable bytes, not `i16` alignment. The production XA prime and refill buffers are allocated as `Vec<u8>`/`Box<[u8]>` at `subsystem.rs:529–538`; their Rust types guarantee only byte alignment. An unaligned destination makes the typed write undefined behavior even if the host allocator usually aligns it. The existing tests use aligned `i16` storage and do not establish this property. **Rule:** BeautifulRust `rust-best-practices` §8, sound unsafe boundaries. **Next step:** characterize exact PCM bytes and partial-write behavior, then use an alignment-safe byte-writing boundary or prove and enforce alignment for every caller. Do not silently change byte order or failure semantics.

### Should investigate: stream creation callback under nonrecursive locks

`file_stream.rs:566–587` holds the lifecycle and stream locks while `create_resource` invokes `source` at `:484`. The production callback in `crates/hp2-engine/src/audio_core_host_stream.rs:35–49` can call lazy-array loading. Reentry into a lifecycle-taking stream operation would deadlock; this audit did not prove such reentry occurs. The current source callback documentation does not state the restriction. `stream_source` invokes `lazy_array_load`; when the saved position is positive, `lazy_array.rs:130–160,514–535` dispatches through the registered serialization body. When it is nonpositive, loading returns immediately. The audio owner rejects same-subsystem reentry (`audio_owner.rs:660–677`), but this does not establish that all direct stream-manager reentry is excluded. **Rule:** BeautifulRust `rust-best-practices` §7, do not call user callbacks under a lock; repository reentry rule. **Next step:** trace lazy-load callbacks and add an explicit reentry/failure contract test before changing lock or callback order.

### Should investigate: partial OpenAL initialization cleanup

`subsystem.rs:240–275` stores an opened device and can return before setting `initialized`; `destroy` returns immediately when `initialized` is false (`:291–294`). Thus this method does not close an opened device after context creation failure, nor its context after later initialization failure. The production `audio_owner::initialize` releases an unused stream manager on `Ok(false)` or an uninitialized error, but does not close these OpenAL handles (`audio_owner.rs:239–275`). `shutdown` calls the same early-returning `destroy`, and `unregister` removes the runtime (`:310–330,681–689`); no `Subsystem` Drop implementation exists in its source. A later `init` overwrites the stored device handle. This establishes the missing cleanup path in the reviewed owner, not an observed device leak in a running game. Whether the legacy failure side effect is intentional remains unverified. **Rule:** BeautifulRust `rust-best-practices` §8, resource ownership and teardown. **Next step:** use a fake host to record failure-point ownership and compare the approved legacy behavior before proposing cleanup.

### Should verify: malformed Vorbis and independent layout evidence

`crates/hp2-codecs/src/vorbis.rs:109–125,171–185,225–277` returns failure on decoder errors, but its non-ignored tests cover missing files and conversion helpers, not truncated Ogg headers, truncated packets, or failure after an already-open stream. `interleave` indexes channel lengths based on the first channel (`:95–103`); equal lengths depend on the decoder contract and were not independently proven here. **Rule:** BeautifulRust `rust-best-practices` §11, adversarial parser tests. Add small malformed/truncated fixtures in a separate approved test change; preserve currently observed open/read/seek behavior.

`crates/hp2-abi/src/lib.rs:52–118` and other ABI modules assert Rust sizes and offsets. These are valuable regression checks, but are not an independent current C++ comparison. Historical probe claims in comments were not independently verified here. `engine/generated.rs` was not fully read. Independently identify the native oracle and compare layout on each claimed platform before calling this parity.

## What to keep

- EA-XA `feed` rejects non-block lengths, null nonempty input and excess requested samples (`eaxa.rs:105–120`). Its partial decode test preserves history and residue behavior.
- DXT1 block input has a fixed eight-byte type; tests cover four-color and transparent byte vectors (`dxt1.rs:30–112`). Preserve integer interpolation and channel order.
- Fire table initialization retains input RNG draw order, explicit floating-point conversions and wrapping fingerprint arithmetic (`tables.rs:17–50`). `FireSession::speed_rand` releases its global lock before trace callbacks (`globals.rs:64–73`). Do not redesign the RNG from style guidance.
- Audio stream teardown cancels queued work and joins the worker before freeing records; stream slots destroy their stream before dropping refill buffers (`file_stream.rs:194–220,440–464`; `subsystem.rs:718–730`). The production audio owner rejects same-subsystem reentry and restores its lease (`crates/hp2-engine/src/audio_owner.rs:660–702`).

## Inventory and review coverage

Every file path in the requested crates is listed below. **Read** means the entire file was read for this audit, not that its behavior is fully verified. **Pending** means not fully read, including files sampled only for headings or search results. Inline tests are part of their source files. No separate crate test files were present at inventory time. The lookup table is checked-in data included by the encoder; its generation provenance was not verified. The generated ABI file is not a hand-edit candidate.

| Crate | Read in full | Pending: not fully read |
|---|---|---|
| `crates/hp2-abi/` | `Cargo.toml`, `src/lib.rs`, `src/engine/mod.rs`, `src/render_particle.rs`, `src/render_scene.rs`, `src/render_span.rs`, `src/render_sprite_filter.rs` | `src/engine/generated.rs`: generated disposition, sampled only, not fully read or qualified |
| `crates/hp2-codecs/` | `Cargo.toml`, `src/lib.rs`, `src/dxt1.rs`, `src/eaxa.rs`, `src/vorbis.rs`, `src/dxt1_encode.rs` | `src/dxt1_encode_lookup.rs`: lookup-data disposition, sampled only, not fully read or qualified |
| `crates/hp2-fire/` | `Cargo.toml`, `src/lib.rs`, `src/fire.rs`, `src/globals.rs`, `src/tables.rs`, `src/abi.rs`, `src/palette.rs`, `src/wave.rs`, `src/wet.rs`, `src/ice.rs`, `src/lifecycle.rs`, `src/sparks.rs`, `src/sparks/redraw.rs`, `src/water.rs` | None |
| `crates/hp2-audio/` | `Cargo.toml`, `src/lib.rs`, `src/al.rs`, `src/file_stream.rs`, `src/host.rs`, `src/openal.rs`, `src/stream.rs`, `src/subsystem.rs`, `src/ambient.rs`, `src/console.rs`, `src/exec.rs`, `src/fade.rs`, `src/lipsync.rs`, `src/listener.rs`, `src/music.rs`, `src/radius.rs`, `src/sort.rs`, `src/vec3.rs`, `src/playback.rs`, `src/priority.rs`, `src/update.rs`, `src/verification.rs` | None |

The nine specified hand-written files and their inline tests were read in full. Generated ABI and lookup data remain explicit generated/data dispositions, not hand-review requirements; provenance remains unresolved. No full-crate soundness, codec, audio or fire review is claimed. The production codec path sampled was `audio_core_host_stream.rs` → `file_stream.rs` → EA-XA/Vorbis. The production OpenAL path sampled was `audio_core_host.rs` → `openal.rs`; initialization, shutdown and unregister bodies were read in `audio_owner.rs`. `fractal_water_owner.rs:375–477` calls the wave and wet kernels with actual mip buffers; `lifecycle.rs:490–520` builds and publishes the palette. Audio `update.rs:220–320,370–430` calls ambient sorting and fades, and `audio_owner.rs:450–550` connects console dispatch to the live subsystem. These caller excerpts do not constitute full-file reviews. No current production `compress_block` caller was found by `rg -n 'compress_block\(' crates --glob '!dxt1_encode.rs'`; absence from this search is not proof of no indirect use.

Generated/data provenance: `engine/generated.rs:1–5` names `Build/gen_engine_abi.py` and a probe manifest, but that script is absent in this checkout, and `git ls-files '*gen*abi*'` returned no match. Only its header and selected declarations were sampled. The encoder identifies four checked-in single-colour libSquish lookup tables and includes `dxt1_encode_lookup.rs`; no generator was established by the bounded source search. Neither data file was rewritten, independently regenerated, or compared against an oracle. Their unread portions are not hand-reviewed. Source comments and the existing fixture do not prove provenance or current ABI parity.

The wet module's introductory comment says short buffers return without writes, but its actual routines assert lengths (`wet.rs:109–118,173–180`); the engine caller passes actual mip lengths. Treat malformed dimensions and failure behavior as a separate characterization task, not an approved change.

## Commands and observed results

Host: Darwin arm64, Rust 1.99.1. Default Cargo features; `openal` was **not** enabled. Tests are `data-none` except that an ignored Vorbis test needs prototype data. The test binary also includes an embedded DXT1 native-derived fixture; this is not a live oracle run.

Artifacts: `out/macos-arm64/Testing/HP2/beautiful_rust/Artifacts/data-audit/run-20261109-120953/`. Each command, log and exit code is retained there. `HOME`, `TMPDIR`, XDG paths and `CARGO_TARGET_DIR` were isolated there. `CARGO_HOME=/Users/mike/.cargo`, `RUSTUP_HOME=/Users/mike/.rustup`, `LC_ALL=C`, `TZ=UTC`.

- `cargo test --locked --offline -p hp2-abi -p hp2-codecs -p hp2-fire -p hp2-audio --all-targets -- --test-threads=1`: exit 0; four test binaries reported 7, 77, 13 and 20 passes, respectively; one codec fixture test ignored. Compiler warnings are in `tests.log`.
- `cargo clippy --locked --offline -p hp2-abi -p hp2-codecs -p hp2-fire -p hp2-audio --all-targets -- -D warnings`: exit 101. First reported failures are four `needless_range_loop` findings in `dxt1_encode.rs:290,391,466,586`. Clippy stopped at the codec failure; this is not a clean result for the other packages.
- `cargo fmt --package hp2-abi -- --check`, `cargo fmt --package hp2-codecs -- --check`, `cargo fmt --package hp2-fire -- --check`, and `cargo fmt --package hp2-audio -- --check`: each exited 1. Logs retain the formatting diffs; no formatting was applied.
- `cargo doc --locked --offline -p hp2-abi -p hp2-codecs -p hp2-fire -p hp2-audio --no-deps`: exit 0.

No SDK-feature, real OpenAL device, audible playback, GPU, CMake, CTest, packaging, Linux, retail, sanitizer, Miri, or independent native ABI comparison was run. No test was retried. The historical fixture and source comments do not establish fresh oracle provenance. Production ownership, callback failure paths, soundness and runtime parity remain unqualified by source inspection. No checks were rerun for this read-only continuation.

Source drift: concurrent changes to `crates/hp2-audio/src/file_stream.rs` and `crates/hp2-codecs/src/eaxa.rs` alter the XA alignment path and add tests. The original alignment finding above records the previously reviewed source and remains open pending the separate worker's review and evidence; the earlier logs do not qualify the changed source. This continuation did not edit either file.

Continuation inspection commands: `git status --short`, `git diff -- Docs/BEAUTIFUL_RUST_AUDIT_DATA.md`, `wc -l` on the nine files, `rg -n` for their production callers, and `git diff --stat` / `git diff` for the two concurrently changed XA files. Relevant caller bodies were read in `fractal_water_owner.rs:300–570`, `fractal_owner.rs:610–725`, `audio_owner.rs:335–445`, `game_tick_owner.rs:1260–1325`, and `music.rs:40–100`. These are source reads, not behavioral checks.
