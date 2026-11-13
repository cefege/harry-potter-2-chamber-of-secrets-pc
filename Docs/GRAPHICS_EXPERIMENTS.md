# Graphics experiments

These changes are not default-enabled or release-accepted. The normal renderer and
stock fallback settings remain supported. This is not a DrawArena sign-off.

## World-only enhancements

Sound/Video "Advanced" has checkboxes for optional enhancements, all default off and
stored in `[WgpuDrv.WgpuRenderDevice]`: `Bloom`, `AmbientOcclusion` ("Soft
shadows"), `ColorGrading` ("Rich color"), `Sharpen`, `FXAA` ("Smooth edges"),
`HighDetailModels` ("Full detail") and `HDR`. They apply live
(`render_device_owner::set_global_quality`). The `HP2_BLOOM` variable is gone.

The owner of the post effects is `crates/hp2-wgpu/src/world_scale.rs`. It stages a
sampleable world color target even at 100% scale, resolves MSAA when required, and
runs one fused pass (`draw::WorldPostPipeline`, `shaders/world_post.wgsl`, plus
`world_post_ao.wgsl` reading the world depth when soft shadows are on) before later
UI draws, so the HUD and menus are never processed. Earlier color draws make that
world scope ineligible: it retains the existing full-resolution path rather than
processing prior UI. Resource and pipeline errors retain the compositor's error path.

`HighDetailModels` forces mesh LOD to full vertex count (`ShapeLODMode` 0, fix 1.0)
in `render_actor_lod_provider.rs`. `HDR` configures an `Rgba16Float` extended-sRGB
surface (macOS EDR) when the display offers it, otherwise SDR; frame capture reads it
back clamped to SDR.

## Fog and draw distance

The Video page's Fog checkbox controls `[WgpuDrv.WgpuRenderDevice] DistanceFog`
(default True). This enables the original volumetric-light and fog-map path, not
a synthetic uniform distance haze. Retail `Adv8Forest`, `Adv3DungeonQuest`,
`Ch6WizardCard` and `Adv6Goyle` contain fog zones and lights with nonzero volumetric radius; their inventory is
retained under `out/task-20261112/Artifacts/fog-draw-distance/retail-fog.jsonl`.
The retained Direct3D source advertises fog maps but not distance fog; the
declared `ZoneInfo.FogDistance` has no established renderer consumer. As in
`UnRender.cpp`, the viewport actor's zone (`Viewport->Actor->Region.Zone->bFogZone`)
gates volumetric fog.

More options → Draw Distance stores `DrawDistance=Original`, `2x`, or `4x` in
the same section. It extends the renderer's perspective far plane (original
65336 units); it does not change fog-light radius, portal visibility, actor
visibility radius or mesh LOD. Fog and farther planes require visual and
performance qualification on retail maps before a parity claim.

On the available Apple M1 Max, hidden-window Entry runs (600 ticks, three
repeats, no fog zone) measured median tick p50 2.807 ms before and 2.735 ms
after. `Ch6WizardCard`, whose start position is inside a fog zone, measured p50
8.44 ms before (fog never drawn) and 9.58 ms after (fog drawn), p95 13.65 → 12.60 ms;
GPU time was not isolated. Fog off/on captures there differ across the whole
frame (`viewclass/Ch6WizardCard-actorzone.png`, `Ch6WizardCard-sheet.png`); the
`Adv8Forest` start is not a fog zone and stays pixel-identical, as the
viewport-actor rule requires. No original-renderer parity capture exists yet.
Evidence is under `out/task-20261112/Artifacts/fog-draw-distance/`.

2026-11-12 checks (macOS arm64, wgpu, retail overlay
`out/task-20261111/Artifacts/load-times/retail-data`): `wgpu_world_scale_gpu`,
`wgpu_surface_round_trip`, `stock_graphics_menu_contracts`, `stock_graphics_settings`,
`ui_toolkit_contracts`, `ui_host_contracts`, `engine_settings_config` and
`engine_settings_policy` passed. Entry frame 110 off/on and the Sound/Video page were
captured and inspected; `Build/perf_baseline.py`-equivalent Entry runs (600 ticks ×3)
showed tick p50 3.3–6.6 ms off and 3.2–3.4 ms on: no CPU cost above noise. GPU time
was not measured separately and HDR output was not observed on an EDR display.
Evidence: `out/task-20261112/Artifacts/graphics-01/`.

### Earlier bloom-only evidence


### Evidence scope

Initial development checks used macOS arm64 on an Apple M1 Max, the wgpu backend,
and disposable copies of an existing retail overlay. Manifest/checksum evidence
establishes overlay self-consistency, not original-media authenticity. The saved
old binary's exact source identity is unresolved. Evidence is retained under
`out/task-20261111/Artifacts/graphics-plan/`.

- Library tests: 90 passed, 75 ignored. Explicit ignored GPU world-scale tests:
  5 passed. The latest named `wgpu_world_scale_gpu` CTest ran 15 tests,
  including bloom resize and format-change coverage, and passed.
- `wgpu_surface_round_trip` and `game_test_contract` passed. The surface test's
  name does not establish real window presentation.
- The first ordinary-clock disabled comparison differed on 298 of 480,000 pixels
  in `Grounds_hub` frame 240, maximum channel difference 2. The other 5 frames
  matched. An old-binary-versus-itself control reproduced that difference. The
  original failure is retained; no tolerance or golden was changed.
- A separate deterministic-clock diagnostic observed real `input=isolated`,
  `audio=loopback`, and `visuals=tick` markers in all 8 runs. Old/old and
  old/candidate-with-bloom-off each matched exactly at frames 30, 120, and 240 in
  `Grounds_hub` and `GreatHall_G`. Bloom-on changed all 6 frames. Those changes
  are expected but are not automatic visual acceptance. The images were inspected.
- These are Rust/Rust controls, not independent native parity. Production clocks
  were not changed. Actor-lighting wall-clock inputs are a plausible source of the
  ordinary-clock instability, but the cause is not proven.
- The renderer smoke tool initially used the removed `hp2_game` filename. Its
  explicit Rust-executable repair reached the game, which exited 0 but presented
  0 frames and reported 300 occluded frames. That initial run was blocked,
  not passed. The tool also needed support for the current adapter log
  marker; synthetic tool contracts preserve the presentation requirement.
- After the owner confirmed that the display was awake, a distinct exact
  `renderer_smoke_wgpu` CTest passed: 300 presented, 0 occluded, 0 outdated, and
  0 other failures. It used the same saved candidate binary, default bloom-off,
  and the configured prototype root. The earlier failure remains retained.
  Evidence is under `out/task-20261111/Artifacts/renderer-awake-check/`. This
  establishes real window presentation for that run, not bloom-on acceptance.

Broader map coverage and visual acceptance remain open. Do not promote these to defaults from these component checks.

## Not implemented

New character-shadow rendering, soft particles, HD texture packs and widescreen menu
artwork remain separate work.
