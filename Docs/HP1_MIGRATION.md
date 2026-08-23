# HP1 Migration Plan — Philosopher's Stone on the HP2 runtime

Goal: one engine (`hp2_game`) that runs both games. HP2 stays exactly as it is;
Harry Potter and the Philosopher's Stone (HP1) becomes a second data profile
(`data-hp1`) backed by the retail ISO. No fork, no remake: HP1 gameplay ships as
UnrealScript packages on the disc and runs on our generic UE1 machinery.

Every fact below is recon-verified; file:line references are in-tree as of
2026-08-23. Full evidence artifacts: `local://hp1-iso-inventory.md`,
`local://hp1-serialization-map.md`, `agent://GameplayScout`.

## 0. Evidence base

### 0.1 The ISO (`/Users/mike/Downloads/HarryPotter1.iso`, US retail, label `HARRY_POTTER_US_3010`, 552 MB)

- Layout: `System/` (HP.exe + 12 DLLs + 29 `.u` + ~25 `.int` + DefUser.ini),
  `Maps/` (41 `.unr`), `Textures/` (58 `.utx`), `Music/` (91 `.umx`),
  `Sounds/` (24 `.uax`), plus `Help/ DirectX/ Support/ setup/ autorun/ save/`
  and localized subdirs (`0/` int, `1/` spa).
- **Package format: FileVersion=76, LicenseeVersion=0, PackageFlags=1**
  (`C1 83 2A 9E` signature). 196 of 242 scanned packages are exactly (76,0,1).
  All HP1-authored content (code `.u`, `.unr` maps, most `.utx/.uax/.umx`)
  is uniformly v76.
- Stock Epic-derived assets are **mixed v61–75**: 61 (Detail/FireEng/GenFX/...),
  68 (FlareFX/FractalFX/UWindowFonts), 69 (16 files), 72 (`Entry.unr` only),
  73, 75. The loader must therefore accept a *range*, not just 76.
- HP1-specific script packages: `HarryPotter.u`, `HPBase.u` (4.3 MB),
  `HPMenu.u`, `HPModels.u` (17 MB), `HPParticle.u`, `HPPuzzle.u`,
  `HPSounds.u` (29.7 MB), `HProps.u` (10 MB), `HPDialog.u`, `DProps.u`,
  `Hog1/Hog2.u`, `Hub2-5.u`, `Tut1-3.u`. Maps: `Lev_Tut*`, `Lev2_*`..`Lev5_*`,
  `Quid_*`, `Snapes_Office`, `Entry.unr`, `startup.unr`.
- `HP.exe` is SafeDisc-packed (`stxt774/stxt371` sections, `secdrv.sys`) —
  irrelevant: we never run it, packages are plain files.
- No `StaticMeshes/` (UE1 predates them), no `.umod`. No encrypted/compressed
  packages. `DefUser.ini` shows HP1 input alias `bBroomAction` and player
  class `Harrypotter.harry`.
- Editor.dll/Editor.u ship on the disc (same editor-runtime subset our build
  already carries — see `Build/CMake/HP2Sources.cmake:109-115`).

### 0.2 Our loader vs v76 (serialization recon)

- Version knowledge is **plumbed but welded**: `ArVer`/`ArLicenseeVer` flow
  through every serializer (`Core/Inc/UnArc.h:209-211`) and old-version
  branches survive (`<=61/63/<64/68`), but entry is gated by constants:
  `PACKAGE_FILE_VERSION=79`, `PACKAGE_MIN_VERSION=60`
  (`Core/Src/Core.cpp:13-14`), `PACKAGE_FILE_TAG=0x9E2A83C1` and
  `PACKAGE_FILE_VERSION_LICENSEE=0` (`Core/Inc/UnObjVer.h:20,30`).
- Summary layout is already correct for v76: Guid+Generations path is `>=68`
  (`Core/Inc/UnLinker.h:148-166`); licensee word is 0 in both games.
- Risk-ranked coupling sites (full table in `local://hp1-serialization-map.md`):
  1. **`FPropertyTag` size encoding** — `Core/Src/UnClass.cpp:101-176` has *no
     version guard at all*; pre-v79 UE1 property tags use a different size
     scheme. Biggest unknown; must be resolved empirically (Phase 1 probe).
  2. **`FObjectExport` ObjectFlags width + `AR_INDEX` serial size/offset**
     (`Core/Inc/UnLinker.h:33-47`) — earlier UE1 wrote narrower flag sets.
  3. Name-entry layout pre-64 vs FString-era (`Core/Src/UnName.cpp:236`) —
     v76 is post-64, likely identical to v79; verify in probe.
- `Build/package79_reference.py` is fully hardcoded: rejects any version ≠ 79
  (`:317`), v79-shaped name/import/export parsers (`:385-460`), UFunction
  native-table derivation documented as v79-specific (`:691`).

### 0.3 Runtime surface (gameplay recon)

- The C++ runtime ships **only generic UE1 Core+Engine natives**
  (`IMPLEMENT_CLASS` sites `Core/Src/UnObj.cpp:785-4148`,
  `Core/Src/UnClass.cpp:282-1370`; Engine `A*` headers). **Zero HP2-specific
  classes exist in C++** — all HP2 gameplay (broom, spells, dialog, UI) lives
  in script packages (`HGame.u` etc.). HP1's equivalent gameplay likewise
  ships on the disc. ~95% of machinery is game-agnostic.
- Full UE1 UnrealScript VM is implemented (`Core/Src/UnCorSc.cpp`, ~3700
  lines; every `EExprToken` in `Core/Inc/UnStack.h:80-189` has a handler).
  Bytecode coverage is complete for HP1 scripts; version differences are
  serialization-only.
- Native dispatch: ordinal table `GNatives[0x1000]` pre-filled with
  `execUndefined` (`Core/Src/UnCorSc.cpp:19`, `:3709-3725`) + name-based
  static binding via `GNativeLookupFuncs` — exactly 16 handlers for Core + 14
  Engine classes (`Launch/Src/HP2StaticPackages.cpp:20-49`).
- Failure modes that matter for HP1:
  - Missing native class → **hard `appErrorf("Can't bind to native class")`**
    (`Core/Src/UnClass.cpp:1099-1101`). Expected first wall.
  - Missing dynamic native → `appErrorf("NotInDll")`
    (`Core/Src/UnObj.cpp:4051-4071`).
  - Unregistered ordinal → `execUndefined` logs critical and **keeps executing
    a desynced frame** (`Core/Src/UnCorSc.cpp:221-228`) — silent corruption;
    our audit must enumerate natives *before* runtime hits them.
  - Unknown tagged property → warn + skip (`Core/Src/UnClass.cpp:604,648-653`)
    — benign.
- Cosmetic HP2-isms in code: title string (`Core/Src/UnVcWin32.cpp:736`),
  crash-reporter paths (`Core/Src/HP2CrashReporter.cpp:274-281`), menu-name
  translation `UnrealQuitMenu`/`UnrealYesNoMenu`
  (`Engine/Src/UnEngine.cpp:283-291`).

### 0.4 Data-profile plumbing (build recon)

- The `-datadir`/`--data-root` path is **already game-agnostic**: a data root
  is any directory with `System/Default.ini` (`Build/game_test.py:283-288`).
  The HP1 root satisfies this shape natively.
- `HP2_TEST_DATA_ROOT` is a CMake cache PATH
  (`Build/CMake/HP2Targets.cmake:4-5`); behavioral tests receive it via
  `-datadir`. Adding `HP1_TEST_DATA_ROOT` mirrors it.
- Single-game hardcodes to parameterize:
  - `Build/smoke_maps.py:47` default data root; `:119-125` `_class_catalog`
    hardcodes `Core.u, Engine.u, HGame.u` (HP1's game package is
    `HarryPotter.u`).
  - `Build/spell_interaction_audit.py:22-26` HP2 spell list + data root.
  - `Build/run_vulkan_smoke.py:34`, `repair_save.py` (HP2 save format —
    out of scope for HP1 initially).
  - Renderer smoke registration gated on `dist/.../HarryPotter2.app` +
    `HarryPotter2/Unreal/System/Default.ini` (`Build/CMake/HP2Targets.cmake:645-669`).
- Label taxonomy already defines data profiles `data-none|data-prototype|data-retail`
  (`Docs/OPERATIONS.md:107-108`); `data-hp1` extends it.

## 1. Strategy

1. **One engine, two profiles.** `hp2_game` gains a `data-hp1` profile; game
   identity (title, packages, config) derives from the data root, not from
   compiled-in HP2 constants.
2. **Load HP1's own script shells.** HP1's `Core.u`/`Engine.u` script parts and
   all `HP*.u` packages load against our static natives — the established
   pattern (stock HP2 packages already import `Editor.Transactor` against our
   editor-runtime subset, `Build/CMake/HP2Sources.cmake:109-115`).
3. **Format layer first, audit-driven after.** A v76-capable loader plus a
   version-parameterized reference tool turns "infer from the maps" into a
   mechanical gap list (missing classes, missing natives, unknown opcodes)
   instead of guesswork.
4. **No silent degradation.** Missing native/class = loud reason code and a
   tracked gap, never `execUndefined` desync (AGENTS.md: no symptom
   suppression).

## 2. Phases

### Phase 0 — `data-hp1` profile + provenance (no runtime changes)

- Extract ISO contents (System/Maps/Textures/Music/Sounds/Help, US English
  only; skip localized subdirs, DirectX, setup, autorun) into an HP1 data
  root, e.g. `HarryPotter1/Unreal` alongside `HarryPotter2/Unreal`.
- Add `Build/prepare_hp1_data.py` (modeled on `prepare_retail_data.py`):
  SHA-256 per file, provenance (ISO label + per-file version/licensee/flags
  from header scan), writes `overlay-manifest.json`-style manifest. Import-time
  validation only (runtime manifest enforcement doesn't exist yet —
  `Docs/OPERATIONS.md:158-166`).
- CMake: `HP1_TEST_DATA_ROOT` cache PATH (`HP2Targets.cmake`), state-file
  records data-root presence (`Build/hp2_state.py`).
- **Gate:** `hp1_package_manifest` ctest (manifest regeneration byte-identical;
  header scan of every package recorded). Runs without runtime changes.

### Phase 1 — v76 format support (critical path)

- **Step 0 — probe before code:** decode a real v76 `FPropertyTag` stream
  (from `HPBase.u` class defaults), name table, and export table bytes.
  Resolves the one true unknown: property-tag size encoding (packed Info byte
  vs explicit size word). Also confirm v76 name-entry layout matches v79.
- Introduce a per-version format table consumed at summary decode
  (`UnLinker.h:117-173`): property-tag scheme, name-entry layout,
  generations/heritage split (already correct ≥68), export ObjectFlags width.
  Accept set becomes `{61..79} ∩ observed` (61, 68, 69, 72, 73, 75, 76, 79);
  `PACKAGE_MIN_VERSION`/`PACKAGE_FILE_VERSION` become the table's bounds, not
  hardcoded equality.
- Parameterize `package79_reference.py` → `package_reference.py` with the same
  table. **v79 output must stay byte-identical** (existing goldens are the
  regression net; any golden bump needs its own acceptance record).
- Licensee hook: `ArLicenseeVer` is plumbed and 0 for both games; keep
  consulted-but-unused, no licensee branches.
- **Gates:** new `hp1_loader_manifest` ctest (audit tool reads all 242 HP1
  packages: name/import/export tables decode, zero format errors); full
  existing suite green (`ctest --preset macos-arm64`) proving no HP2 behavior
  change.

### Phase 2 — Class inventory + native gap list (audit-driven)

- Extend the package audit to emit, per HP1 package: script classes, their
  native parents, `FUNC_Native` declarations with ordinals, default-property
  references. (Same derivation `package79_reference.py:691` does for v79,
  generalized.)
- Diff against our native surface (`GNatives` registrations +
  `HP2StaticPackages.cpp` handlers). Output: machine-readable gap list —
  missing native classes, missing native functions, layout-drift suspects.
- Load HP1 `Core.u`/`Engine.u` shells + `HarryPotter.u` + `HPBase.u` in a
  test harness (`-datadir` HP1 root): every class must `Bind` or appear in the
  gap list. Fix order: (a) implement missing Engine-era natives in our static
  handlers, (b) extend the 16-handler list for HP1 packages, (c) for
  layout-drifted RF_Native classes, align our headers to HP1-era layout *per
  class* — never by disabling Bind checks.
- **Gate:** `hp1_class_bind` ctest — zero unbound classes in HP1 base
  packages; gap list shrinks to explicitly-accepted entries with reason codes.

### Phase 3 — Map boot + render + audio

- Boot order: `startup.unr` (menu) → `Entry.unr` → `Lev_Tut1.unr` via
  `game_test.py run --data-root <hp1 root>`.
- Expected failure classes, each with a pre-planned response:
  import resolution (`Paths` list for HP1 root), missing classes (Phase 2
  leftovers), missing natives at call time (implement; loud reason code if
  deferred — never silent).
- Renderer: HP1 textures are UE1-era palettized + DXT (our GL driver already
  handles HP2's identical era); `UWindowFonts` v68 stock. Bitmap-font
  compatibility mode stays as-is (AGENTS.md rule 5).
- Audio: `.uax` → EAXA decoder exists (`Core/Src/FEAXABlockDecoder.cpp`);
  `.umx` (91 Soule tracks) via existing music path + vendored vgmstream.
  Verify both under HP1 data; codec work only if format variants differ.
- Menus: `HPMenu.u` on stock `UWindow.u` (v68). Check
  `UnEngine.cpp:283-291` menu-name special cases don't misroute HP1 menus.
- **Gate:** `hp1_smoke` ctest — commit-gate tier (3 maps × 120 ticks, same
  failure-marker policy as `renderer_smoke_xopengl`), registered only when
  HP1 root + app bundle exist (same configure-time convention as
  `HP2Targets.cmake:645-669`).

### Phase 4 — Gameplay systems (inferred from data, audit-driven)

- Enumerate HP1 systems from the Phase 2 inventory + map scans: spell set
  (Diffindo/Flipendo-era, differs from HP2's list), `bBroomAction` broom
  flight, `HPDialog.u` dialog/cutscene flow, Quid pitch variants
  (`Quid_*A-C`), `Hub2-5.u` hub logic, `Tut1-3.u`.
- For each system: audit which script functions/natives the maps actually
  call, implement or verify the native backing, then exercise via
  input-script runs (pattern: `Tests/InputScriptSmoke.py`).
- Save/load: HP1 save format differs from HP2's; defer until maps boot, then
  add `hp1_save_format` contract (no reuse of `repair_save.py`).
- **Gate:** per-system behavior tests + `hp1_smoke` extended tier; each system
  documented in `Docs/BEHAVIOR_MATRIX.md` under `data-hp1`.

### Phase 5 — Unification + gate promotion

- Parameterize remaining single-game assumptions (`smoke_maps.py`
  `_class_catalog` takes game package name from profile; spell audit becomes
  profile-aware; title/crash-reporter strings derive from data root).
- One smoke harness, two profiles; `data-hp1` label applied to all HP1 tests.
- Feature-gate promotion per AGENTS.md rule 4: `data-hp1` profile starts
  `experimental`; `runtime-verified` on Phase 3 evidence (per-map boot
  artifacts); `retail-verified` on full-sweep evidence (all 41 maps, 300
  ticks); `default-enabled` is a separate, explicit decision.
- **End state:** `hp2_game -datadir <hp1-root> startup.unr` plays the game;
  `ctest --preset macos-arm64` covers both profiles; HP2 suite untouched-green
  throughout.

## 3. Cross-cutting rules (from AGENTS.md, binding)

- Every phase lands with: invariant, exact command, observed result, unresolved
  conditions. Data-profile honesty: HP1 claims hold only under `data-hp1`.
- No golden updates without separate acceptance records; no retry-to-green;
  no symptom suppression (unknown native ≠ skip).
- `ThirdParty/` untouched (vgmstream/UT469eSDK already vendored cover HP1's
  formats; no pin changes anticipated).
- Native text stays `experimental`; HP1 fonts (US English: stock UWindowFonts)
  don't change that.

## 4. Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| v76 property-tag encoding differs from both known schemes | Medium | Phase 1 step 0 probe on real bytes before any code |
| HP1-era Engine class layout drift breaks RF_Native Bind | High (expected) | Per-class alignment driven by audit; never disable checks |
| Native gap long tail (HP1 ordinal/name space) | High | Audit-generated list; loud reason codes; no silent `execUndefined` |
| Mixed stock versions (61–75) hide format variants | Low-Medium | Loader accepts table-driven range; audit scans every package |
| `.umx` music variants pre-v75 | Low | Verify under HP1 data in Phase 3; vgmstream fallback |
| Scope creep in Phase 4 gameplay inference | High | Audit-driven: implement only what maps/scripts reference |

## 5. Unresolved conditions (explicit)

- v76 property-tag/name-entry encodings: unverified until Phase 1 probe.
- HP1 script-shell Bind compatibility against our natives: unknown until
  Phase 2 audit.
- HP1 save format: not investigated; scheduled after map boot.
- Localizations (Spanish/Hungarian/Japanese assets, `PolFont.exec`,
  `SAPFont.*`): out of scope; US English profile only.
- SafeDisc: moot (packages are plain; `HP.exe` never executed).
- Runtime `overlay-manifest.json` enforcement doesn't exist for any profile
  (`Docs/OPERATIONS.md:158-166`); HP1 inherits import-time-only validation.

## 6. First three actions (in order)

1. Phase 0: extract ISO → HP1 data root + `prepare_hp1_data.py` + manifest.
2. Phase 1 step 0: v76 header/property-tag probe (Python, read-only) —
   decides the format table's shape.
3. Phase 1: loader version table + `package_reference.py` parameterization,
   HP2 suite green + `hp1_loader_manifest` passing.
