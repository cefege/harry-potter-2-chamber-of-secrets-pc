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

Median of 5 runs x 600 ticks, M1 Max, uncapped. All 10 maps now meet the 60 fps target (p95 tick time
at most 16.67 ms) with margin; before, 8 of 10 missed it by up to 2.7x.

| Map | baseline p50 ms | + draw arena | + engine caches | total speedup | fps@p50 now | p95 ms now | p99 ms now | RSS MB before→now |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| PrivetDr | 9.31 | 9.14 | 3.24 | 2.9x | 308 | 3.84 | 5.63 | 380→347 |
| Entry | 3.28 | 3.29 | 2.00 | 1.6x | 500 | 2.44 | 2.67 | 329→319 |
| Ch2Skurge | 22.77 | 19.33 | 6.83 | 3.3x | 147 | 8.02 | 9.11 | 545→465 |
| Adv1Willow | 22.42 | 15.38 | 6.52 | 3.4x | 153 | 7.54 | 8.36 | 581→466 |
| Entryhall_hub | 28.48 | 25.38 | 8.24 | 3.5x | 121 | 10.38 | 13.52 | 622→510 |
| Grandstaircase_hub | 31.60 | 28.40 | 9.35 | 3.4x | 107 | 12.01 | 26.20 | 710→527 |
| Grounds_hub | 35.02 | 27.41 | 10.00 | 3.5x | 100 | 11.01 | 11.74 | 529→393 |
| GreatHall_G | 34.28 | 19.63 | 12.09 | 2.8x | 83 | 13.36 | 14.12 | 661→621 |
| Quidditch | 20.30 | 12.00 | 5.84 | 3.5x | 171 | 8.34 | 8.90 | 676→482 |
| Duel01 | 18.00 | 10.87 | 6.81 | 2.6x | 147 | 8.63 | 9.60 | 584→468 |

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
Also run: all 222 `fast` CTests (pass) and the 42-map retail sweep (42/42 clean).

## Disclosure

A profiling run on 10 Nov used the real HOME for about ten minutes before `perf_profile.py` was fixed
to isolate it; the game rewrote `~/Library/Application Support/Harry Potter 2/User/HarryPotter2.log`
(a log file; no ini or save changed by timestamp). The previous log content was not preserved.
