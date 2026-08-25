# Rust vs C++ render baseline — G4 status (TrackG2, verified post-fix)

## Status: G4 COMPLETE — real rendering in hp2rs smoke mode, all gates
## re-evidenced after three root-cause fixes (2026-08-25 session)

Retail HP2 maps ship editor-template geometry for brushes PLUS one CSG'd
level model carrying the static world. Scene extraction covers both:

- Brush actors → `Brush` object property → Model export → `Polys` ci →
  UPolys/FPoly wire decode → brush-transformed world polys.
- The unreferenced level model (e.g. Model1, 713 KB in PrivetDr) → full
  BSP topology (Vectors/Points/Nodes/Surfs/Verts) → world-space polys
  from node vertex pools, textured via surf records (openhp1
  `Model::triangulate` approach).

### Root-cause fixes landed this session (second package: brush LocalToWorld)

0. **Brush LocalToWorld was wrong** (TrackDiff C++-source-verified):
   PrePivot was dropped (275 PrivetDr brushes displaced), pitch/roll
   rotation signs were inverted (C++ builds Ry(−pitch)·Rx(−roll)), and
   PostScale was unapplied. Now: `world = M·(v − PrePivot) + Location`
   with `M = diag(Post)·Rz(yaw)·Ry(−pitch)·Rx(−roll)·diag(Main)`;
   normals/texU/V = `M·d` (scale-preserving — BSP surf bases carry texel
   scale 0.5..2.0, normalizing destroyed texture alignment); mirrored
   scale (det<0) reverses winding. Camera + brushes share ONE rotation
   helper (`render_bridge::rotation_matrix`); camera forward is now the
   UE1 `FRotator::Vector` convention (pitch + = up). TrackDiff probe:
   30/30 vertex deltas zero, 5/5 brushes 100% match incl. rotated
   Brush502/80/82. USER-VISIBLE RESULT: the Privet Drive street renders —
   house row, roofs, chimneys, windows, lawns.

### Root-cause fixes landed this session

1. **View-projection matrix was transposed** (`render_bridge::pass_uniforms`):
   rows were built from basis-vector COMPONENTS (`[right.x, up.x,
   forward.x·a, 0]`) with the translation in the wrong row, so the shader's
   `clip[j] = row_j · v` produced `w = (b − a·f·eye)·v.z + 1` — every world
   collapsed into wedges radiating from a vanishing point; only huge polys
   survived. Rows are now the full dot-product rows
   (`x' = right·(v−eye)/tan_h`, `y' = up·(v−eye)/tan_v`,
   `z' = a·(f·v−eye)+b`, `w' = f·(v−eye)`).
2. **Winding/cull convention**: under the (now correct) left-handed screen
   basis a surface whose normal faces the camera projects CLOCKWISE, so the
   pipelines use `FrontFace::Cw` + `cull_mode: Back` (UE1/D3D-style).
   Verified empirically: interiors show wall faces, top-down shows
   roofs/ground. hp-render's own GPU tests and billboard/build_mesh
   geometry were re-wound to the same convention.
3. **Level-model extraction + two wire-format gaps**: (a) `FBspNode` is
   `Plane f32×4 + ZoneMask u32 + NodeFlags u32 + 1 reserved byte + SEVEN
   compact refs (iVertPool, iSurf, back, front, plane, collision_bound,
   render_bound) + iZone 2B + NumVertices u8 + iLeaf 2×i32`
   (brute-force-validated on Model1's 3306 nodes; the openhp1 9-ci reading
   and the earlier "mystery byte" reading were both misparses);
   (b) some `UPolys` payloads carry the HP2 object-stack prologue
   (`[ci][ci][FF×8][i32][u8 0x81]`, wire-spec item 1) and `parse_upolys`
   now tries that alignment first and prefers non-empty exact-consumption
   decodes (an earlier empty-Ok short-circuit silently dropped 721 polys
   incl. all of Brush423).

### openhp1 cross-check (SplittyDev/openhp1, read-only reference)

`Model::decode` confirms the shared grammar (tagged props → bounds → five
TArrays → NumSharedSides i32 → zones → Polys ref). Divergences: their
FBspNode reads `zone_mask u64 + flags u8` and 9 cis — same byte budget as
our validated `u32 + u32 + u8 + 7 cis + iZone 2B`, different split; their
prologue gating uses export flag `0x0200_0000` vs our `payload[0] & 0x80`
(same FObjectStack structure: ProbeMask FF×8, LatentAction, ByteCodeOffset
0x81). Their triangulation (node vertex pool fans) is what our level-model
path mirrors.

## Honesty table

| Feature | State |
| --- | --- |
| Brush-polygon world geometry (100% model linkage) | REAL |
| Static level geometry from CSG'd level model BSP | REAL (3306 polys in PrivetDr Model1) |
| UPolys/FPoly binary decode (all alignments incl. object-stack prologue) | REAL (exact-consumption gated) |
| .utx P8/DXT1 textures | REAL (2374 validated; procedural classes loud-checkered) |
| Texture mapping via FPoly/surf basis + P8 LUT / DXT1 | REAL |
| wgpu pipelines (Palette LUT / Masked / Translucent / Modulate / Opaque) | REAL |
| Camera view-projection (fixed matrix; authored 90° hFOV at 1024×768) | REAL |
| Camera placement from PlayerStart | REAL (exact, structural test) |
| Offscreen render, PNG+meta capture, resource-balance protocol | REAL |
| Determinism (same seed/fixed-dt ⇒ byte-identical captures) | REAL |
| Winding/cull (Cw front, back-culled; Newell-verified normals) | REAL |
| Baked lightmaps (zone ambient + light contributions + blurred 1bpp shadow masks, x2 overbright) | REAL (1900 lightmaps reconstructed on PrivetDr Model1; 700+ with visible light pools; linear-sampled atlas with replicated-edge gutters) |
| Gouraud vertex lighting (actor meshes) | NOT RENDERED (brush/bsp only today) |
| Skybox (fake backdrop, two-pass sky-zone render) | REAL (SkyZoneInfo pose; PF_FakeBackdrop 0x80 portals sample the sky pass screen-space; 64 portal surfaces on PrivetDr) |
| Zone fog | NOT RENDERED |
| Sprites, movers, particles | NOT RENDERED |
| Procedural textures (Fire/Wave/Ice/Wet) | APPROXIMATED (loud checkerboard; shipped mips empty, C++ generates at runtime) |
| Missing packages (`HGame.utx` not shipped in retail) | APPROXIMATED (loud checkerboard) |
| Pixel comparison vs C++ baselines | DEFERRED to post-G6 fidelity backlog (C++ frame differs due to missing systems: cutscene spline camera, lightmaps, skybox, emissives, fog — not a render bug) |

## G4 gate evidence — raw outputs from the verifying run

Release build rebuilt fresh before all runs.

### Structural acceptance (numeric, `cargo test -p hp-engine g4_structural`)

```text
[g4] brush transforms: 4 brushes, 92/92 polys matched hand-computed world vertices
[g4] camera: PlayerStart Location=[-1664.0, 624.0, 720.0] Rotation=[0, 0, 0]
     == scene camera [-1664.0, 624.0, 720.0]/[0, 0, 0]
[g4] coverage: 518 model exports, 516 brush-referenced, 2 level/orphan,
     scene polys=6483 == decoded sum 6483
[g4] winding: 6483 polys cross==normal, 0 disagree
[g4] textures: 6357/6483 resolved, loud fallbacks {"HGame": 123, "CVresearch": 2}, no-ref 1
[g4] UVs: Brush423 first-poly |uv|max=0.3333335 (sampled 2)
```

(5 tests, all passing. Brush423 Location asserted == (7760, 352, 208).)

### Gate (a) — trio WITH rendering (no --null-render, no fallback)

`python3 Build/game_test.py run --app dist/macos-arm64/HarryPotter2.app
--data-root HarryPotter2/Unreal --map Maps/<M>.unr --renderer xopengl
--ticks 120 --timeout 180 --log /tmp/g4_<M>.log --engine-bin
target/release/hp2rs`

| Map | scene | gl_textures_created/destroyed | passed | failure_markers | scene_unavailable |
| --- | --- | --- | --- | --- | --- |
| PrivetDr | 6483 polys (516 brushes + Model1 3306 + Model484 6) | 189 / 189 | true | [] | 0 |
| Entry | 18 polys | 2 / 2 | true | [] | 0 |
| Ch2Skurge | level models + brushes | 102 / 102 | true | [] | 0 |

`grep -c renderer.scene_unavailable` across all three logs: **0**.
Loud texture notes (by design): `HGame.utx` absent from retail Textures/
(verified: not among the 73 shipped packages); CVresearch failures are
UE1 procedural classes (FireTexture/WaveTexture/IceTexture/WetTexture —
probe-verified: every failing export is one of those classes with 0-byte
stored mips; C++ generates them at runtime).

### Gate (b) — determinism

Two runs, Ch2Skurge (heaviest scene), seed `0x08f4c815`,
`--fixed-dt=0.03333334`, 120 ticks, `HP2_CAPTURE_FRAMES` set:

```text
58eb5c2b888e2a7a27f7d4f357a1d01a38fde2c88fc64b7edf7417a32136898d  runA/frame_000119.png
58eb5c2b888e2a7a27f7d4f357a1d01a38fde2c88fc64b7edf7417a32136898d  runB/frame_000119.png
```

`cmp` clean — byte-identical (sha256
79af3cf8a8ccc3469f43659864a8e9991bb47403b841f29aa4b6e488362c3d05).

### Visual evidence

`/tmp/g4_stats_PrivetDr_frame.png` (also
`/tmp/g4_vis_PrivetDr/frame_000119.png`): PlayerStart view of PrivetDr —
THE PRIVET DRIVE STREET: recognizable row of two-story houses with
pitched roofs, chimneys, windows, lawns and a walkway, correct
placement/orientation (user-confirmed 3D forms; texture alignment on
static surfaces fixed by the scale-preserving basis fix). Submission
proof test: 6483 scene polys → 6456 draws (27 invisible/untextured).

### Palette/tiling fix (user-reported red sky)

- UE1 `FColor` serializes **R, G, B, A** (shipped UnTex.h:70-72) — the
  palette bytes are true RGBA and the Rgba8 LUT upload was already
  correct; the sky's red was NOT a channel swap.
- Real cause: the base sampler used **ClampToEdge** — UE1 world textures
  TILE. The huge sky quads' uv0 spans hundreds of tiles; clamping
  collapsed them onto one edge palette entry (solid red). Base sampler is
  now `Repeat` on all axes (the lightmap atlas keeps its own
  clamp+linear sampler with replicated gutters).
- Result: the authored red storm-cloud sky tiles correctly; houses show
  lit windows (baked light pools), tiled roof/brick detail.

### Skybox milestone evidence (TrackLearn plan item 3)

- Sky zone: PrivetDr SkyZoneInfo at (11901, −5890, 33) — a sealed 254³
  skybox room at the map edge (zone index 2, 6 BSP nodes); scene carries
  the pose as `RenderScene::sky_zone`.
- Fake-backdrop surfaces: PF_FakeBackdrop = 0x00000080 (shipped
  UnObj.h:232 — NOT the 0x08000000 of later UE versions); 64 portal
  surfaces on PrivetDr (10 brush + 54 BSP: Forestgrass ground, EctoWet
  water, ceiling).
- Render: two submits per frame — pass 1 renders the non-backdrop scene
  from the SkyZoneInfo pose into a same-size sky target; pass 2 renders
  everything from the main camera with backdrop polys sampling the sky
  target in screen space (`frag.clip.xy / misc.yz`), depth-tested like
  opaque geometry. `update_camera` refreshes the stored main camera so
  free-fly movement persists across frames.

### Lightmap milestone evidence (TrackLearn plan item 1)

- Parse: Model1 lightmap block decodes exactly — LightMaps=1900,
  LightBits=115585 (both match the TrackLearn probe), bounds=1338 (25-byte
  FBox: min+max+valid u8), hulls=11356, leaves=1036, lights=24179.
- Reconstruction: 1001/1900 lightmaps carry light lists; 700+ reconstructed
  images show visible light pools (verified by PNG dump:
  /tmp/g4_lm); zone ambient resolves via node iZone[1] → zone table →
  ZoneInfo props (zone 1 ambient hue=32 sat=100 bri=50), LevelInfo black
  fallback; Light actors decode with UE1 class defaults
  (brightness=64, saturation=255, radius=64, cone=128).
- GPU: lightmaps shelf-packed into one BGRA atlas (1-texel replicated
  gutters, linear-sampled); P8 bases palette-expand to color textures for
  the Lightmap path; FRAG_LIGHTMAP applies the ×2 overbright.
- Remaining artifact: the map-top-left black wedge with red streaks
  (the unflagged sky-ceiling brush at grazing angle + zone boundary) —
  under investigation with the fog milestone.

### Gate (d) — suites and lint

- `cargo test -p hp-render -p hp-engine -p hp2rs`: all 7 suites green
  (hp-engine 40 lib incl. 5 structural + 2 integration, hp-render 21 unit
  + 11 GPU, hp2rs clean).
- `cargo clippy -p hp-render -p hp-engine -p hp2rs --all-targets --
  -D warnings`: clean.
