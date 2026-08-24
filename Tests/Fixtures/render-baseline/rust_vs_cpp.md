# Rust vs C++ render baseline — G4 status (TrackG2, verified)

## Status: G4 COMPLETE — real rendering in hp2rs smoke mode, all gates
## evidenced by direct runs on this branch (option-a-publishable)

Retail HP2 maps ship **editor-template geometry**: ~518 `Engine.Model`
exports and **zero** prebuilt BSP (`Nodes`/`Surfs`/`Verts` exports absent).
Each brush's polygons live in root-level `UPolys` exports (`outer=0`,
names like `Polys76`). World geometry = brush polygons transformed by the
owning brush actor's `Location`/`Rotation`/`MainScale`.

### Verified wire facts (this branch)

- `FPoly` wire (HP2 deltas vs stock): compact NumVertices; Base/Normal/
  TexU/TexV f32×3; verts; u32 PolyFlags; compact Actor ref; compact
  Texture ref; **one compact ItemName index (no number word)**; compact
  iLink/iBrushPoly; i16 PanU/PanV. Validated by exact-payload-consumption
  (`parse_upolys`, gated on consuming the payload exactly).
- UModel → Polys linkage (validated on all 517 brush models across the
  trio, 100% linkage): optional binary object-stack prologue (present iff
  `payload[0] & 0x80`): `[ci class_ref][ci class_ref][FF×8][i32 V][u8
  0x81]`; then tagged properties to `None`; then Bounds f32×6, i32,
  9 reserved bytes, f32 C, five empty TArray counts (Vectors/Points/
  Nodes/Surfs/Verts — retail maps ship no BSP), `NumSharedSides` i32,
  `NumZones` i32 (+ 17 B/zone), then **one compact object reference to
  the root-level `Polys` export** (1-based, 1:1 model↔polys).
- Texture refs resolve through import outer chains to shipped `.utx`
  packages (HP2_Master, PrivetDrive, SkyBox, CVresearch, GenFX);
  2374/2374 shipped content textures decode as P8+palette or DXT1.
- Brush `Location`/`Rotation`/`MainScale`/`PostScale` live on the brush
  actor's tagged properties (`Brush` object property → Model export →
  Polys ci → geometry); placement structs decode by declared struct name.

### openhp1 cross-check (SplittyDev/openhp1, read-only reference)

`crates/openhp1-map/src/model.rs::Model::decode` independently confirms
the same grammar on HP1-era data: tagged props → bounds →
Vectors/Points/Nodes/Surfs/Verts TArrays → `shared_side_count` i32 →
zones (ref + u64 connectivity + u64 visibility = 17 B) → **Polys object
reference**. Divergences observed (HP1 vs HP2 or interpretation):

- FBspNode: openhp1 reads `zone_mask: u64` + `flags: u8` and **9**
  compact refs (incl. `zones[2]`) before `vertex_count: u8`; we read
  `i32 + i32 + 3 mystery bytes` and 7 refs. Same byte budget; moot for
  retail HP2 maps (node arrays are empty). Left as-is, documented.
- Prologue gating: openhp1 gates its object-stack read on export flag
  `0x0200_0000` and models it as UE1 `FObjectStack` (function/state
  refs, ProbeMask u64, LatentAction i32, ByteCodeOffset u8). We gate on
  `payload[0] & 0x80` and treat the block as opaque — the `FF×8` is an
  all-ones ProbeMask, so both readings agree structurally.
- openhp1 validates trailing LightMaps/LightBits/leaf data to exact
  consumption; we stop after the Polys ci (retail trailing bytes are
  zeros + G/H filler, not needed for brush extraction).

## What renders today

The full pipeline runs on retail maps: brush-polygon meshes built once
per map from FPoly wire facts (FPoly-basis UVs, fan-triangulated),
textured via P8 LUT palette pipeline or DXT1, camera view-projection
from `PlayerStart` placement with authored 90° horizontal FOV at
1024×768, offscreen Metal target, PNG + `frame_meta.json` capture.
hp2rs defaults to tick-end rendering with `--null-render` opt-out,
`--render-every=N`, `HP2_CAPTURE_FRAMES` capture, and loud reason-coded
degradation (`renderer.scene_unavailable`) when scene extraction fails —
which fired **zero** times across the gate trio.

## Honesty table

| Feature | State |
| --- | --- |
| Brush-polygon world geometry (trio: 100% model→Polys linkage) | REAL |
| UPolys/FPoly binary decode | REAL (byte-exact, exact-consumption gated) |
| .utx P8/DXT1 textures | REAL (2374 validated; see texture notes below) |
| Texture mapping via FPoly basis + P8 LUT / DXT1 | REAL |
| wgpu passes incl. Palette LUT / Masked / Translucent / Modulate | REAL |
| Camera projection (authored FOV, PlayerStart placement) | REAL |
| Offscreen render, PNG+meta capture, resource-balance protocol | REAL |
| Determinism (same seed/fixed-dt ⇒ byte-identical captures) | REAL |
| Lightmaps / gouraud lighting | APPROXIMATED (fullbright white) |
| Zone fog | NOT RENDERED |
| Sprites, movers, particles | NOT RENDERED |
| Procedural textures (FireTexture/WaveTexture/IceTexture/WetTexture) | APPROXIMATED (loud checkerboard; UE1 generates these at runtime, shipped mips are empty) |
| Missing packages (`HGame.utx` — not shipped in retail) | APPROXIMATED (loud checkerboard) |

## G4 gate evidence — raw outputs from the verifying run (2026-08-24)

Release build: `cargo build --release -p hp2rs -p hp-engine -p hp-render`
(fresh, no stale binary; rebuilt before the runs).

### Gate (a) — trio WITH rendering (no --null-render, no fallback)

`python3 Build/game_test.py run --app dist/macos-arm64/HarryPotter2.app
--data-root HarryPotter2/Unreal --map Maps/<M>.unr --renderer xopengl
--ticks 120 --timeout 180 --log /tmp/g4_<M>.log --engine-bin
target/release/hp2rs`

| Map | scene | camera (PlayerStart) | gl_textures_created/destroyed | passed | failure_markers |
| --- | --- | --- | --- | --- | --- |
| PrivetDr | 2450 polys | (-1664, 624, 720) | 183 / 183 | true | [] |
| Entry | 12 polys | (48, -16, -80) | 2 / 2 | true | [] |
| Ch2Skurge | 9766 polys | (-1.32, 1309.45, 819.04) | 78 / 78 | true | [] |

`grep -c renderer.scene_unavailable` across all three logs: **0** (real
GPU rendering, no null degradation). Texture notes (loud, by design):
PrivetDr only — 12× `HGame.utx` absent from retail Textures/ (verified:
not among the 73 shipped packages), 1× CVresearch P8 with empty stored
mip. A probe over CVresearch.utx shows every failing export is a UE1
procedural class — FireTexture/WaveTexture/IceTexture/WetTexture
(firetut1, WaveT, wet155, ICE2, …) whose shipped base mips are 0 bytes;
the C++ engine synthesizes them at runtime. Entry/Ch2Skurge: zero notes.

### Gate (b) — determinism

Two runs, Ch2Skurge (heaviest scene), same seed `0x08f4c815`,
`--fixed-dt=0.03333334`, 120 ticks, `HP2_CAPTURE_FRAMES` set:

```
1e9775e5514f3fbdb97f6d056778585549fb9a30a2138a11f38a5c951ddf6ed6  runA/frame_000119.png
1e9775e5514f3fbdb97f6d056778585549fb9a30a2138a11f38a5c951ddf6ed6  runB/frame_000119.png
```

Byte-identical (`cmp` clean; frames 000000 identical too — geometry is
static under the smoke tick loop).

### Pixel comparison vs C++ baselines (advisory)

`python3 Tools/baseline_compare.py
Tests/Fixtures/render-baseline/<M>/frame_000119.png <rust
frame_000119.png>` (final frames, 1024×768 both sides):

| Map | max_channel_delta | verdict |
| --- | --- | --- |
| PrivetDr | 219 | differs |
| Entry | 255 | differs |
| Ch2Skurge | 255 | differs |

Expected to differ: the Rust path renders fullbright unlit brush
geometry with loud checkerboards for procedural/missing textures, while
the C++ baseline includes lightmaps, gouraud lighting, sprites, movers
and runtime-generated procedural textures. Advisory for human review,
per plan.

### Gate (d) — suites and lint

- `cargo test -p hp-render -p hp-engine -p hp2rs`: all suites pass
  (hp-engine 34 lib + integration, hp-render 21 unit + 11 GPU pipeline
  tests, hp2rs clean).
- `cargo clippy -p hp-render -p hp-engine -p hp2rs --all-targets --
  -D warnings`: clean.
