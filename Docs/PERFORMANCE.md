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

Baseline (`base1`, 5 repeats x 600 ticks) versus after the batched draw arena (`arena4`):

| Map | before p50 ms | after p50 ms | before p95 | after p95 | speedup (p50) | after fps@p50 | meets 60 fps (p95 ≤16.67) |
|---|---:|---:|---:|---:|---:|---:|:--:|
| PrivetDr | 9.31 | 9.14 | 10.67 | 9.95 | 1.02x | 109 | yes |
| Entry | 3.28 | 3.29 | 3.96 | 4.60 | 1.00x | 304 | yes |
| Ch2Skurge | 22.77 | 19.33 | 27.06 | 21.20 | 1.18x | 52 | no |
| Adv1Willow | 22.42 | 15.38 | 26.17 | 16.90 | 1.46x | 65 | no |
| Entryhall_hub | 28.48 | 25.38 | 35.06 | 28.08 | 1.12x | 39 | no |
| Grandstaircase_hub | 31.60 | 28.40 | 44.87 | 31.32 | 1.11x | 35 | no |
| Grounds_hub | 35.02 | 27.41 | 37.57 | 29.24 | 1.28x | 36 | no |
| GreatHall_G | 34.28 | 19.63 | 36.52 | 21.03 | 1.75x | 51 | no |
| Quidditch | 20.30 | 12.00 | 32.53 | 14.62 | 1.69x | 83 | yes |
| Duel01 | 18.00 | 10.87 | 19.52 | 13.30 | 1.65x | 92 | yes |

Before the change 8 of 10 maps missed 60 fps (up to 2.7x short). The profile of GreatHall_G showed
about half of main-thread time inside wgpu command recording: one render pass, two new GPU buffers,
three bind groups and a 12-28 KB zero-padded uniform upload per batch.

## Change 1: `draw::DrawArena` (hp2-wgpu)

`dispatch_batches` now collects a flush's draws and replays them in one render pass with vertex and
draw-parameter data packed into two buffers created per flush. Two hazards found by pixel comparison
and fixed: reusing one buffer pair across back-to-back flushes let a later write overtake the earlier
submission on Metal, and overlapping draw-parameter windows changed frames. Output is not bit-identical
to the old path: blending that used to round through an 8-bit store between passes now stays in tile
memory. Against the pre-change build, 20 of 27 captured frames are pixel-identical, the other 7 differ in
at most 321 pixels (0.07%), with a few edge pixels differing by up to 81/255. Treat it as a visual
tolerance change that needs your sign-off, not as a proven-identical refactor.

## Disclosure

A profiling run on 10 Nov used the real HOME for about ten minutes before `perf_profile.py` was fixed
to isolate it; the game rewrote `~/Library/Application Support/Harry Potter 2/User/HarryPotter2.log`
(a log file; no ini or save changed by timestamp). The previous log content was not preserved.
