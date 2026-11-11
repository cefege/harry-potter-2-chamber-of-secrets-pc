# Graphics experiments

These changes are not default-enabled or release-accepted. The normal renderer and
stock fallback settings remain supported. This is not a DrawArena sign-off.

## World-only bloom

Start a newly built game process with `HP2_BLOOM=1` to request a subtle highlight
halo. Unset the variable, or use a value other than `1`, to leave bloom off. There
is no in-game setting yet. The choice is read when the render loop is created.

The owner is `crates/hp2-wgpu/src/world_scale.rs`. It stages a sampleable world
color target even at 100% scale, resolves MSAA when required, and composites the
world through `draw::BloomPipeline` before later UI draws. The shader is
`crates/hp2-wgpu/src/shaders/bloom.wgsl`. Earlier color draws make that world scope
ineligible: it retains the existing full-resolution path rather than processing
prior UI. Resource and pipeline errors retain the compositor's error path.

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

Performance, broader map coverage, and visual acceptance remain open. Do not promote bloom to a default from these component checks.

## Not implemented by this experiment

New character-shadow rendering, soft particles, ambient occlusion, and widescreen
menu artwork remain separate work. Existing stock effects are not evidence that
these upgrades are complete.
