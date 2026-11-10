# Runtime performance program

Goal: every retail map at 60 fps or better on Apple Silicon, measured uncapped. Rule 14 of
`AGENTS.md` applies: every claim below has a measurement behind it and says what it does not cover.

## Method

* `HP2_FRAME_TIMING=1` makes the engine log per-tick `frame_ms` and a `<HP2_PERF>` summary
  (tick cost = wall time of `game_engine_owner::tick`, which includes rendering CPU work and any GPU
  waits inside the tick). Opt-in; off by default.
* `Build/perf_baseline.py` runs maps for a fixed number of ticks, several repeats, one fresh process
  per sample, and reports median and min/max of p50/p95/p99/max tick time, load time, peak RSS.
  Every sample is forced uncapped: engine ini `FrameRateLimit=1000` (a 1000 fps ceiling, never
  reached), `UseVSync=Off`, and `--fixed-dt` so the unfocused-window 10 fps clamp cannot engage.
  A sample fails if more than 25% of the frame lies outside the tick (a cap detector); this caught
  that clamp (102 ms frames around a 22 ms tick) on the first attempt.
* `Build/perf_profile.py` + `Build/sample_summary.py`: macOS `sample` stack profile of one map
  (isolated HOME), summarized to main-thread inclusive/self time and subtrees.
* `Build/frame_equiv.py`: captures PNG frames at fixed indices and compares decoded pixels between
  two builds. Baseline run-to-run noise is up to about 16 pixels on a few frames.
* Machine: Apple M1 Max, macOS, retail overlay copy (see `NO_LAUNCHER_CUTOVER.md`), 800x600 window,
  `-nosound`. Not measured: other hardware, physical display pacing, audio, input latency.

## Results

Median of 5 runs x 600 ticks, M1 Max, uncapped. All 10 maps meet 60 fps (p95 tick <= 16.67 ms); before, 8 of 10 missed it by up to 2.7x.

| Map | before p50 ms | now p50 ms | speedup | fps@p50 now | p95 ms now |
|---|---:|---:|---:|---:|---:|
| PrivetDr | 9.31 | 3.40 | 2.7x | 294 | 7.36 |
| Entry | 3.28 | 2.71 | 1.2x | 368 | 8.27 |
| Ch2Skurge | 22.77 | 6.37 | 3.6x | 157 | 7.84 |
| Adv1Willow | 22.42 | 6.88 | 3.3x | 145 | 8.26 |
| Entryhall_hub | 28.48 | 7.29 | 3.9x | 137 | 9.42 |
| Grandstaircase_hub | 31.60 | 8.21 | 3.8x | 122 | 10.85 |
| Grounds_hub | 35.02 | 9.15 | 3.8x | 109 | 10.26 |
| GreatHall_G | 34.28 | 12.57 | 2.7x | 80 | 13.95 |
| Quidditch | 20.30 | 6.37 | 3.2x | 157 | 8.89 |
| Duel01 | 18.00 | 7.12 | 2.5x | 140 | 9.40 |

The first profile of GreatHall_G showed about half of main-thread time inside wgpu command recording
(one render pass, two new GPU buffers, three bind groups and a 12-28 KB zero-padded uniform upload per
batch). After that was fixed, the profile of Grounds_hub showed about 60% of the frame in four engine
hot spots: name interning (23%), clearing references to deleted actors (14%), and two reflected
property lookups (22%).

## Change 1: `draw::DrawArena` (hp2-wgpu)

`dispatch_batches` now collects a flush's draws and replays them in one render pass with vertex and
draw-parameter data packed into two buffers created per flush. Two hazards found by pixel comparison
and fixed: reusing one buffer pair across back-to-back flushes let a later write overtake the earlier
submission on Metal, and overlapping draw-parameter windows changed frames. Output is not bit-identical
to the old path: blending that used to round through an 8-bit store between passes now stays in tile
memory. Against the pre-change build, 20 of 27 captured frames are pixel-identical, the other 7 differ in
at most 321 pixels (0.07%), with a few edge pixels differing by up to 81/255. Treat it as a visual
tolerance change that needs your sign-off, not as a proven-identical refactor.

## Change 2: engine caches (hp2-core, hp2-engine)

* `CoreAccess::native_class` memoizes successful lookups (it did three name interns and three object
  lookups per call); cleared in `pre_exit`.
* `property_value` no longer interns None/Vector/Rotator/Plane on every call.
* `CoreAccess::property_slot` caches reflected property offsets per class; the dynamic-actor render
  path used to walk the class hierarchy by name for two properties of every actor every frame.
* `cleanup_destroyed_direct` computes each class's object-reference slot offsets once, in exactly the
  order the old field walk visited them, instead of walking every field of every actor on every tick
  while any actor is pending deletion.

Evidence this changed no simulation logic: `Build/rng_equiv.py` compares the full RNG-consumption trace of
two builds on fixed-step runs. The trace is byte-identical between the draw-arena build and this one on
Entry (29 MB), Grounds_hub (33 MB) and Adv1Willow (56 MB). Pixels at one captured frame (Grounds_hub 240)
differ in 0.45% of pixels by at most 20/255 against the previous build while identical to itself across runs;
rendering that depends on wall-clock time is the likely reason (frames got faster), not verified further.
Also run: all 222 `fast` CTests (pass) and the 42-map retail sweep (42/42 clean). Final build: 42/42 clean again, 220 of 223 fast CTests plus the three `native_free_*` audits pass after the LTO revert.

## Change 3: startup and small wins

* Overlay verification hashed every file of the retail overlay (455 MB, software SHA-256) on one thread
  at every launch, about a second. It now hashes on up to 8 threads and reports the first failure in the
  same entry order, so errors are unchanged. Launch-to-exit of a 5-tick run: Entry 2.97 -> 1.91 s,
  Grounds_hub 3.48 -> 2.41 s.
* `core_native_catalog::by_opcode` is a table lookup instead of a scan of the whole native catalog on every
  VM native call (small measured gain).
* Fat LTO + one codegen unit: 2-6% faster and 23% smaller, but it breaks the `native_free_*` audits
  (Apple `nm` cannot read the LLVM bitcode), so it is not enabled; see the note in `Cargo.toml`.

## Change 4: save, load and level switching

Measured with `Build/saveload_bench.py` (Adv1Willow retail map, replay `input_script_save_reload.txt`:
F5 = `SAVEGAME 9`, F9 = `LOADGAME 9`; stall = worst `frame_ms` in the tick window; median of 3 runs,
macOS arm64, wgpu, retail data copy). Frame-time logging is `HP2_FRAME_TIMING=1`, which now also
prints a per-operation breakdown of every `LoadMap` (`<HP2_PERF> load_phase`).

| Operation | Before | After |
| --- | --- | --- |
| Save game (game stall) | 336 ms | 18 ms |
| Save game (blocking, `HP2_SYNC_SAVE=1`) | 336 ms | 263 ms |
| Load saved game | 826 ms | 420 ms |
| First map load at startup (`LoadMap`) | 390 ms | 293 ms |

* **Background save.** `SAVEGAME` `fork`s. The child is a copy-on-write snapshot of the process and
  runs the unchanged blocking `save_game_with` (no GPU, no progress screen, no logging, stdout and
  stderr to /dev/null, `_exit`). The parent applies the live-state updates the blocking save made
  after writing (brush tracker, destroyed-actor cleanup, mover positions, `LEVACT_NONE`, cache flush)
  and returns. The next save, `LOADGAME`/`Browse`, `LoadMap`, the `RestartSounds`/cache-copy exec path
  and shutdown first wait for the child (`finish_pending_save`); a child that fails or exceeds
  30 s is replaced by a blocking save. `HP2_SYNC_SAVE=1` restores the blocking path.
  Risks: forking a multithreaded process is only safe because the child touches no GPU/SDL state and
  allocates; a lock held by another thread at fork time could hang the child (the timeout then
  falls back). Stress runs (repeated saves 3 ticks apart, save followed by load 2 ticks later, on two
  maps) passed with no stalls. `Tests/SaveAsyncEquivalence.py` (`save_async_equivalence`) proves the
  slot files equal the blocking save's except the 16-byte random package GUID (also different between
  two blocking runs) and that the stall is under half the blocking stall.
* **Hashing.** `hp2_core::fast_hash` (FxHash-style) replaces SipHash in the save tables and class/
  access maps; name comparison in the property archive no longer allocates. Blocking save 336 -> 263 ms.
* **Load sort.** `sort_loaded_from` read three dependent pointers per comparison. Keys are now read once
  per object and sorted with the same quicksort and the same decisions. Load/startup 390 -> 293 ms.
  RNG traces on Entry, Grounds_hub, Adv1Willow and Quidditch are byte-identical to the build before it.
* **Fade-out shortened (visible change).** Before a level is replaced, `LoadMap` draws a fade-out for
  `Console.FadeoutTime` seconds of wall time (~0.5 s in retail), it is now 25% of
  that (`HP2_LOAD_FADE_SCALE=1` restores the original; any value in (0, inf) is accepted).
* **Cutscene skip.** Not a load operation: holding Space during a cutscene (debug mode only, `HPConsole`)
  calls `SloMo(8.0)` and fast-forwards the cutscene threads, i.e. it runs game time at 8x. It is as fast
  as the per-frame cost allows; the map-load speedups above apply when a cutscene ends in a level change.
  Not changed.
* Remaining load cost (Adv1Willow load, 420 ms): EndLoad ~140 ms (package preload), garbage collection
  ~128 ms, fade ~125 ms.

## Game speed

Normal play is real time: the loop uses the measured frame time, capped at 0.1 s per tick, times the
level's `TimeDilation`. `--fixed-dt`/`-testticks` exist only for tests. `HP2_FRAME_TIMING=1` now also prints
`game_speed` (game seconds per wall second). `Build/speed_probe.py` on PrivetDr with a focused window:
1.00 at frame limits 30, 60, 144 and 1000. `Build/dt_probe.py`: Quidditch, Entryhall_hub and the title scene
look the same at equal game time at 30 and 144 fps. Frames over 100 ms run in slow motion (clamp), and an
unfocused window is capped to 10 fps by the engine.

## Disclosure

A profiling run on 10 Nov used the real HOME for about ten minutes before `perf_profile.py` was fixed
to isolate it; the game rewrote `~/Library/Application Support/Harry Potter 2/User/HarryPotter2.log`
(a log file; no ini or save changed by timestamp). The previous log content was not preserved.
