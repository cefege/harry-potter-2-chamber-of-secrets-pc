# UI completion checklist

This is the numbered implementation and qualification backlog for the **in-game menus, HUD and console-level presentation**. Source integration and exhaustive runtime qualification are separate: the finite toolkit views are integrated, while the original game scripts retain gameplay, lifecycle, actions and dispatch.

**2026-11-12 architecture correction:** the rejected timer/health/combat/aux
body installers and health-scope probe are disconnected from `ui_hud::install`.
The complete base spell selector now uses a Rust view model, `hp2-ui` image/text
nodes and the existing host bridge (`ui_spell_selector.rs`). Gameplay remains
script-owned. UI-20–22 remain open. A distinct stock control with
`HP2_STOCK_HUD=0` now reproduces the same loss-cutscene CutError. All four
active-selector frames (450/510/570/600, selection 1→2→3→1) are PNG-byte-identical
between stock and toolkit at 800×600, including readable 1/2/3 labels. Both
full scenarios still FAIL qualification; cutscene frames 630 and 700 differ,
and neither difference is waived. No cue investigation or unchanged retry ran.
Four exact CTests and the shipping build passed in the preceding implementation
scope, after its retained compile failure was corrected. Evidence:
`out/task-20261112/Artifacts/hud-selector-01/HANDBACK.md` and
`out/task-20261112/Artifacts/hud-selector-stock-02/`.
Historical current-host/counter descriptions below are superseded, not new
implementation instructions. Shared Core additions remain; no blanket rollback
or full-HUD acceptance is claimed. Counts are unchanged.

### Whole-HUD source cutover — bounded delivery gate, 2026-11-12

**User-approved delivery gate:** finish the replacement UI, verify normal reachable screens/interactions, and report exact blocked edge cases; do not keep investigating or repairing unrelated engine/cutscene failures. This changes qualification depth, not feature scope. Decision: `out/task-20261112/Artifacts/whole-hud-01/delivery-authorization.json`. The exhaustive row gates below remain an honest follow-on qualification ledger, not prerequisites for this bounded handback.

UI-48 and UI-50 have actual 800×600 bitmap/native-text acceptance. The other integrated rows retain their explicit edge gates; implementation is not missing merely because a qualification row remains open. No reachability exclusion, feature-default promotion or independent native-parity claim is implied.

- Seven individually selected CTests and the shipping build passed in
  `families-04`: `ui_toolkit_contracts`, `ui_host_contracts`,
  `ui_hud_contracts`, `canvas_compatibility_contracts`,
  `console_vm_native_owner_contracts`, `render_console_contracts`, and
  `scripted_texture_vm_native_owner_contracts`. This includes a real-Core,
  actual-Canvas empty-caption cursor/font regression, not a mock draw-call test.
- Player health and the ordinary Bean-room timer each matched stock across
  five complete decoded RGBA frames. LLDB hit the real Rust health/timer
  callbacks and both inspected processes continued to exit0. These bounded
  captures do not qualify low/empty/healing/hold/fade, enemy health, numeric
  tuning, gold-card or expiry states.
- Whole finite source-level components are integrated for selector, magic,
  countdown, health, status groups, vendor, pickups, popups, Quidditch,
  challenges, lessons, cutscene text, console presentation and auxiliary
  candidates. ScriptedTexture house-score plaques also compile. Source
  integration is not evidence that their real gameplay entries ran.
- Shared pickup `DrawActor` now uses the canonical typed native owner rather
  than silently invoking an indexed native as a script event. Vendor/page
  texture reads preserve their original positions relative to reentry.
  Review also corrected integer console clipping and cutscene painter lookup
  order/null-centering behavior. Runtime qualification remains open.
- First failures are retained: `families-01` rejected a console UTF-32/String
  mismatch; `families-02` exposed an incorrect exact-fit boundary test input;
  `families-03` rejected an unaligned reference in the new packed-Canvas test.
  Each succeeding run followed a specific source correction; none erased
  the first failure.
- Selector's seven size/provider pairs retain all49 complete-frame comparisons
  and remain failed/open. NativeText=True/backend readiness was observed, but
  actual font identity was not logged. An additional `console-startup-01`
  pair matched frames10/30/50/90 and differed at140. It used the stock smoke
  gameplay fixture, not a proved main-menu interaction fixture. That requested
  frame lies after the logged127-frame replay; uncontrolled tail timing is
  under investigation, not waived.
- Rejected exclusive timer/health/combat/aux/sports walkers, contracts and
  mechanics tests are removed, including the orphan lesson-admission test.
  Independently consumed Core callback/statement facilities and Canvas
  fixtures remain; blanket shared-Core deletion is not authorized.
- The final prepared-overlay census checks all488 indexed entries,
  104packages and42maps, with6607 decoded script spans and no package/script/
  default decode failures. Static completeness does not establish dynamic
  reachability. The newly identified Weapon overlay and house-score plaques
  receive UI-51/UI-52 below rather than disappearing inside inventory closure.
- The user explicitly authorized moving only canonical Weapon.RenderOverlays'
  inline render-coupled muzzle timing into its complete finite Rust adapter.
  Firing/damage/input policy, native movement/rendering and callback ordering
  remain unchanged. This is an architectural exception, not runtime acceptance.

Evidence and exact commands:
`out/task-20261112/Artifacts/whole-hud-01/` (`SHARED-RESULT.md`,
`SELECTOR-RESULT.md`, `families-01/` through `families-04/`,
`health-entry-01/`, `countdown-bean-01/`, `route-debug-01/`,
`route-debug-02/`, `console-startup-01/`, `entry-matrix/`,
`weapon-boundary/authorization.json`).
Platform: macOS arm64, Release sdl/openal/renderer-wgpu/text-coretext;
retail captures use the prepared488-entry overlay, not authenticated media.
CTest data-none/prototype results are not retail interaction qualification.


## Counts

- **18 groups with functional evidence:** 13 menu/dialog pages, 3 HUD primitives, and the newly qualified ShowPos/TimeDemo views. The16prior groups were not all rerun during this checklist audit.
- **32 presentation groups with remaining qualification gates:** finite components are source-integrated; per-row interaction, lifecycle, resource and visual edges remain recorded below. These are not32unwritten interfaces. Reachability gaps are not silent exclusions.
- **2 final gates:** inventory closure and end-to-end acceptance.

**52 numbered entries; 18 with functional evidence and 34 open.** These are delivery groups, not equal-sized functions or a weighted completion percentage. No trustworthy finish date is established. Entries42–43 are closure gates; entries44–45 were added by the first audit,46–50 by the decoded console follow-up, and51–52 by the complete source inventory, without renumbering earlier IDs. Any additional discovered UI must receive its own explicit row rather than disappear inside a completion claim.

Menu inventory is grounded in the12explicit `PAGES` entries in `crates/hp2-engine/src/ui_mirror.rs`, plus the hand-written Sound/Video page in `ui_pages`. HUD inventory is grounded in the retained actual local-package inventory at `out/task-20261111/Artifacts/finish-wave1/hud/full-hud-inventory-01/INVENTORY.md`. That package identity is local-overlay evidence, not authenticated original media. Historical documentation can predate current partial HUD work.

## What closes an exhaustive qualification entry

1. Its intended drawing/layout policy runs through the real production entry. Test-only bridges, compilation and generic canonical replay alone do not close it.
2. Its listed visible states and interactions work. Keep canonical gameplay state, callbacks, warnings, partial effects and ownership behavior unchanged.
3. Relevant exact behavioral tests pass. Exercise the actual game route and inspect retained frames; compare with stock drawing without hiding differences or refreshing goldens implicitly.
4. Preserve bitmap text and the stock override. An opt-in or sampled component-only result stays partial until the matching runtime acceptance and any separately required promotion decision.
5. Record the command, actual result, profile, executable/source identity and evidence path on this entry. An unresolved failed required comparison keeps it open.

The existing menu design intentionally retains stock page artwork and stock game actions while Rust owns controls/input. Finishing the UI does not mean rewriting gameplay scripts or retail packages.

## Implemented menus/dialogs — 13

| ID | Element | Existing implementation / final regression to retain |
|---|---|---|
| UI-01 | Main/startup menu (`FEMainPage`) | Mirrored Rust controls/input. Recheck every actual start/profile/load/navigation/quit action present on the page. |
| UI-02 | Pause/book hub (`FEInGamePage`) | Mirrored Rust controls/input. Recheck open, resume, navigation and every actual save/load/exit action present. |
| UI-03 | Options hub (`FEOptionsPage`) | Mirrored controls. Recheck page navigation and Back. |
| UI-04 | Controls/key bindings (`FEInputPage`) | Mirrored controls. Recheck rebinding, mouse values, keyboard/text editing and persistence. |
| UI-05 | Sound/Video settings (`ui_pages`) | Hand-written Rust page. Recheck settings, apply/confirmation, persistence and restored values. |
| UI-06 | Folio/card collection (`FEFolioPage`) | Mirrored controls; stock artwork retained. Recheck category/item/page selection and descriptions. |
| UI-07 | House points page (`FEHousepointsPage`) | Mirrored controls. Recheck displayed values, navigation and Back. |
| UI-08 | Quidditch menu page (`FEQuidPage`) | Mirrored controls. Recheck choices, actions and Back; this is NOT the gameplay HUD. |
| UI-09 | Duelling menu page (`FEDuelPage`) | Mirrored controls. Recheck choices, actions and Back; this is NOT the duel HUD. |
| UI-10 | Map page (`FEMapPage`) | Mirrored controls. Recheck actual available markers, actions and Back. |
| UI-11 | Credits (`FECreditsPage`) | Mirrored controls. Recheck display and navigation. |
| UI-12 | Language grid (`FELangGrid`) | Mirrored controls. Recheck selection and resulting text/settings. |
| UI-13 | Message/confirmation dialogs (`HPMessageBox`) | Mirrored controls. Recheck accept, cancel, close, focus and underlying action. |

These rows mean implemented with previous evidence, not a new50-entry regression pass. All remain in the final regression gate.

## Implemented HUD primitives — 3

| ID | Element | Existing implementation / boundary |
|---|---|---|
| UI-14 | Subtitle/card-description text drawing | Rust wrapping, centering and shadow. Full manager/state closure remains UI-37. |
| UI-15 | Letterbox/border drawing primitive | Rust border and existing slide drawing. Full cutscene manager interactions remain UI-37. |
| UI-16 | Ordinary status icons and counts | Rust base item/count drawing: beans, potions, keys, cards and similar counters. Group placement/visibility remains UI-36; health is UI-17. |

## Integrated HUD views — remaining qualification, 22 groups

| ID | Element | Current state | Required to close |
|---|---|---|---|
| UI-17 | Player health | TOOLKIT INTEGRATED — EDGE GATES OPEN | `ui_player_health.rs`; ordinary Entry health frames match stock. Low/empty/healing/maximum and hold/fade transitions, partial-tile/resource edges and the retained BeanRewardRoom failure remain unqualified. |
| UI-18 | Enemy health | TOOLKIT INTEGRATED — EDGE GATES OPEN | `ui_enemy_health.rs`; actual full/partial/empty, concrete head variants, appearance and disappearance remain unqualified. Start/End and gameplay remain script-owned. |
| UI-19 | Magic-strength meter | TOOLKIT INTEGRATED — EDGE GATES OPEN | `ui_magic_strength.rs`; actual strength transitions, visibility and numerical/Canvas effects remain unqualified. No added clamp. |
| UI-20 | Spell-selector layout | TOOLKIT IMPLEMENTED — ACCEPTANCE OPEN | Whole base view integrated; four active-selector frames at 800×600 match stock byte-for-byte through selection 1→2→3→1. Full scenarios fail on the same CutError; supported sizes and complete lifecycle remain unqualified. |
| UI-21 | Spell icons | TOOLKIT IMPLEMENTED — ACCEPTANCE OPEN | All three selected/unselected images use toolkit nodes and the Canvas bridge; sampled stock comparison is exact. Custom subclasses/callback routes remain stock. Missing-resource and additional scaling contracts remain open. |
| UI-22 | Hotkey labels/text | TOOLKIT IMPLEMENTED — ACCEPTANCE OPEN | Whole-view label nodes replace the rejected X/Y interpreter migration. All three labels and selection changes match stock exactly in four 800×600 frames, with NativeText=False. Stock-presence override =0 is exercised. Font-provider identity, other sizes, Canvas-state and complete lifecycle qualification remain open. Both full runs retain the same loss-cutscene CutError; do not restart the VM/cue-probe project. |
| UI-23 | Quidditch power/progress bar | TOOLKIT INTEGRATED — EDGE GATES OPEN | `ui_quidditch_bar.rs`; actual values, flashing/color, display/idle and numerical behavior remain unqualified. |
| UI-24 | Quidditch score/tally/hold | TOOLKIT INTEGRATED — EDGE GATES OPEN | `ui_quidditch_score.rs`; actual score changes, tally, house-point tally, post-tally hold and cutscene interactions remain unqualified. |
| UI-25 | Challenge score/tally/points | TOOLKIT INTEGRATED — EDGE GATES OPEN | `ui_challenge_score.rs`; actual active/tally/hold/points-hold states remain unqualified. |
| UI-26 | Spell-lesson instructions/round/result | TOOLKIT INTEGRATED — EDGE GATES OPEN | `ui_spell_lesson.rs`; real DADA approach now starts the authored intro, but complete rounds/results/visibility remain unqualified. Existing entry-cutscene errors are retained, not repaired. |
| UI-27 | Standard countdown timer | TOOLKIT INTEGRATED — EDGE GATES OPEN | `ui_countdown.rs`; ordinary countdown frames match stock. Idle/expiry, callback/override and Canvas edges remain unqualified. |
| UI-28 | Bean-room timer variant | TOOLKIT INTEGRATED — EDGE GATES OPEN | Ordinary BeanRewardRoom timer has actual pictures; duration changes/expiry remain unqualified. Gameplay policy stays canonical. |
| UI-29 | Gold-card timer variant | TOOLKIT INTEGRATED — EDGE GATES OPEN | Gold graphics/override are integrated in `ui_countdown.rs`; actual initialization/start/stop/restart/expiry remain unqualified. |
| UI-30 | Timer tuning/debug display | TOOLKIT INTEGRATED — EDGE GATES OPEN | Numeric/tuning drawing is integrated; ordinary timer pictures do not establish its actual activation or callback branches. |
| UI-31 | Vendor interface | TOOLKIT INTEGRATED — EDGE GATES OPEN | `ui_vendor.rs` covers VendorTransaction, MakePurchase, NotEnoughBeans and DrawVendorBar; successful/failed purchases, inventory effects and transitions remain unqualified. |
| UI-32 | Flying/registered pickup display | TOOLKIT INTEGRATED — EDGE GATES OPEN | `ui_pickups.rs` retains typed native DrawActor; none/one/multiple pickups, depth, registration/removal and draw order remain unqualified. |
| UI-33 | Spell-page popup | TOOLKIT INTEGRATED — EDGE GATES OPEN | `ui_popups.rs`; actual content, overlap, dismissal and deletion remain unqualified. |
| UI-34 | Warning popup | TOOLKIT INTEGRATED — EDGE GATES OPEN | `ui_popups.rs`; actual text, overlap, dismissal and deletion remain unqualified. |
| UI-35 | Popup dispatch/lifecycle drawing | TOOLKIT INTEGRATED — EDGE GATES OPEN | Complete popup drawing coordination is integrated; actual virtual subtype calls, post-draw deletion and cleanup still need lifecycle qualification. |
| UI-36 | Status-group layout/visibility | TOOLKIT INTEGRATED — EDGE GATES OPEN | `ui_status_groups.rs`; ordinary items have evidence, but group ordering/Idle visibility and vendor dependencies remain unqualified. |
| UI-37 | Cutscene manager/state layout | TOOLKIT INTEGRATED — EDGE GATES OPEN | `ui_cutscene.rs` and existing subtitle/border views preserve the deliberate wrap/placement, contrast and time-based slide. Actual DADA intro pictures exist. Duel01 stock loss-cue errors and retained630/700differences remain blockers, not authority to rewrite gameplay. |
| UI-38 | Overall HUD mode/order/visibility | FINITE CHILD VIEWS INTEGRATED — EDGE GATES OPEN | Script-owned dispatch/state keeps the original manager order and conditions; admitted child drawing is toolkit-owned. Complete menu/cutscene suppression, cross-manager ordering and lifecycle qualification remains open. |

## Reachability decisions — 3

| ID | Candidate | Required decision |
|---|---|---|
| UI-39 | Muggle-detection meter | `ui_muggle_meter.rs` is integrated. Actual retail caller/instance and rise/fall/idle evidence remain missing. User kept this open; no exclusion accepted. |
| UI-40 | Hoop drawing (`HPHud.DrawHoops`) | Finite `ui_extra_hud.rs` view is integrated. Actual callers/states remain missing; not called by the bounded PostRender source. User kept this open. |
| UI-41 | Additional HUD spell-icon entry (`HPHud.DrawSpellIcon`) | Separate finite `ui_extra_hud.rs` view is integrated. Actual caller/visibility evidence remains missing; this is not the selector icon. User kept this open. |

## Final gates — 2

| ID | Gate | Required result |
|---|---|---|
| UI-42 | Complete the UI entry/override inventory | The newer existing reader decoded HPConsole.PostRender and PlayerPawn.PostRender; old opcode/setup blocks are resolved without a new decoder. PlayerPawn adds HUD dispatch, not another group. Console trees added UI-46–UI-50. Still OPEN on actual Engine/UWindow state/override/subclass and external dynamic reachability. Names or a complete static walk do not prove visible state. Retain ui-inventory-closure-01/02 logs and give further confirmed UI its own row. |
| UI-43 | End-to-end UI acceptance | Run all menu interactions and all reachable HUD routes, inspect and compare frames, verify bitmap text, supported UI sizes/aspects, stock overrides, UWindow root/console gating and cursor/input behavior, and no duplicated drawing. Exercise actual actions and persistence using private data/saves. Retain platform/feature/profile limitations; obtain any separately required default-promotion acceptance. No automatic release/tag/packaging operation is authorized. |

## Existing Rust overlays found by the inventory audit — 2

| ID | Element | Current state | Required to close |
|---|---|---|---|
| UI-44 | FPS overlay | NORMAL-FLOW SMOKE — EDGE GATES OPEN | Existing `game_tick/draw.rs::fps_block` is not an unimplemented port. Final direct native smoke shows positive66top-right, initial nonpositive suppression and config-off suppression. Exact post-replay frame78differs by66pixels because only the toolkit capture has a positive FPS value; retained, not masked. Live checkbox interaction and complete Canvas-state qualification remain open. |
| UI-45 | Recording/replay status text | NORMAL-FLOW VERIFIED — EDGE GATES OPEN | Final real `-RECORD` creates a private native `.rep`, real `-REPLAY` consumes it, and inactive mode omits the bottom indicator. All9record/active-replay/inactive full-RGBA pairs match stock. Pause/single-step/physical-stop and wider font/size qualification remain open. |

The bounded audit found no omitted group in the retained HGame dispatch mapping and confirmed the12mirrored pages plus handwritten Sound/Video page. It did not close the console, PlayerPawn, external-entry or dynamic-subclass questions. No game, compiler or native test ran in that audit. `engine-walk.log` is empty and is not a successful walk; the worker reported setup failure.

## Additional console-level presentation and candidates — 5

| ID | Element | Current state | Required to close |
|---|---|---|---|
| UI-46 | Loading, paused and level-action presentation | TOOLKIT INTEGRATED — EDGE GATES OPEN | Finite `ui_console_views.rs` covers the canonical messages/background pieces; actual loading/pause/save/connecting/precache branches and state interactions remain unqualified. |
| UI-47 | Console presentation/input and message views | TOOLKIT INTEGRATED — EDGE GATES OPEN | Finite console drawing and canonical input/message ownership are integrated in `ui_console_views.rs`/`console_owner.rs`; full console access, scroll/message-state and input interaction qualification remain open. |
| UI-48 | ShowPos coordinate text | VERIFIED — 800×600 BITMAP/NATIVE | Real private supported debug config and ShowPos exec exercise off/on/off, negative-coordinate/integer text and simultaneous TimeDemo. All18complete RGBA pairs across the three console scenarios match stock; HP2_STOCK_HUD=0 preserves stock by presence. See console-acceptance.json. Other sizes/aspects and exact native-font identity remain UI-43 gates. |
| UI-49 | Demo-lockout Locked text | FINITE VIEW INTEGRATED — REACHABILITY OPEN | `ui_console_views.rs` covers DrawE3DemoLockout; no supported actual bE3DemoLockout activation is proved. User kept it open; ordinary play is not claimed to reach it. |
| UI-50 | Timedemo presentation | VERIFIED — 800×600 BITMAP/NATIVE | Real TimeDemo True/False execs exercise off/on/off, AVG and LAST SEC updates with original Calc/FormatFloat callbacks. Two separate verified private copy-on-write profiles also produce identical canonical40-byte System/fps.txt outputs; original overlay is not an output destination. Shared typed GetEntryLevel boundary repair and retained first SIGSEGV are documented below. Broader UI-43 gates remain open. |

Source evidence is `ui-inventory-closure-02/RESULT.md` and its decoded trees/logs. No runtime or visual check ran in that follow-up. The known count increased because these groups were omitted from the initial bounded inventory, not because new UI features were requested. UI-42 remains open.

## Additional mixed and off-Canvas presentation — 2

| ID | Element | Current state | Required to close |
|---|---|---|---|
| UI-51 | Weapon overlay muzzle image and actor continuation | FINITE ADAPTER INTEGRATED — ACCEPTANCE OPEN | Canonical inherited Engine.Weapon.RenderOverlays includes inline muzzle start/expiry/reset. The user explicitly authorized this narrow render-coupled timing exception; see weapon-boundary/authorization.json. Preserve actual declared CalcDrawOffset, native movement, false/false DrawActor, Canvas effects, strict expiry and partial failures. Actual first-person receiver, positive flash trigger/MFTexture resources and runtime equivalence remain unproved. |
| UI-52 | House-score ScriptedTexture plaques | TOOLKIT IMPLEMENTED — ACCEPTANCE OPEN | CeremonyManager.RenderTexture measures/rasterizes the actual bitmap font into four live ScriptedTextures through the canonical native owner, with live integer midpoints/color and actual status getters. Preserve script initialization/timer/NotifyActor/teardown. Entryhall actor4099 and static material resources do not prove a visible update; retain actual callback, pixels and value-change/lifetime evidence. |

## Exhaustive follow-on qualification order

The user-approved bounded UI delivery gate above supersedes this order as a prerequisite for handback. Retain these entries for explicit future qualification; do not restart gameplay-route archaeology or engine/cue repairs as hidden UI scope.

1. **UI-20–UI-22 edge acceptance**, using the integrated menu-style toolkit implementation, not the disconnected VM-level candidates. Four active-selector frames exactly match stock at800×600; other sizes, Canvas-state/resource edges and complete lifecycle remain open. The known CutError occurs with stock too; no renewed cue diagnostics.
2. **Close UI-17** health, including the outstanding visual failure and missing state/path evidence.
3. **Close UI-27–UI-30** timer family as one bounded family, with each row reported separately.
4. **Close UI-18–UI-21** remaining combat drawing, one row at a time.
5. **Close UI-23–UI-26** sports/challenge/lesson drawing, one row at a time.
6. **Close UI-36, then UI-31–UI-35** status-group dependencies, vendor, pickup and popups.
7. **Close UI-37–UI-38**, resolve UI-39–UI-41 and run the existing-overlay regressions UI-44–UI-45.
8. **Close UI-46**, resolve UI-47–UI-50 and finish UI-43. UI-42 must already have its inventory result or named unresolved entries from the parallel audit.

Native work remains parent-serialized. A real blocker can change this order, but it must be attached to the affected numbered row. Pause unrelated LTO, graphics effects, compiler experiments, performance and warning work. Infrastructure work must name the UI row it directly unblocks; it is not another completed UI element.

## Current evidence and reporting

Latest focused run: `out/task-20261111/Artifacts/finish-wave1/hud/hotkey-production-integration-01/EXECUTION-21-01/`. All six individual exact CTests report1/1; each selected Rust child reports1passed/0failed/0ignored. The actual loop matches canonical `(8,23)` normally and `(9,23)` with X-extent aliasing; this bypasses retail body entry and does not prove game visuals. This is macOS arm64; five data-none cases and one prototype-data case. Shipping build22 then passed once: exact `cmake --build --preset macos-arm64 --target cargo-build_hp2_game_rust --parallel 2`; actual shell0, no primary/secondary failure, no safety/process latch. Parent file checks verified1246sources,7before/finalcopies and9immutable inputs. Game16709680bytes/SHA `c46815f568572277443ac1e8708bb5ff7c5dd870230e49d2ed5fa71d14491818`; Cargo release duration48.44s. Normal CMake self-regeneration was permitted, not suppressed; equal initial/final graph hashes do not prove no regeneration. Actual Duel01capture23 then FAILED `CutError observed`:2prefixed lines identify the HarryLoses context and unfinishedDuellistcue in one diagnostic block. Maintainedsmoke exited0, but that is not qualification.1251HotKeybodymarkers/7frames retained; Pi inspected originals; initial uncertainty was resolved by enlarged crops confirming visible1/2/3. See preserved initial observation plus `VISUAL-ADDENDUM-23.json`. This does not erase the CutError failure or prove the selected X/Y paths. Full1246sources/488overlayentries/22immutableinputs/7before+7finalcopies stayedconsistent. No control or unchanged rerun ran; authority23consumed. See `hotkey-production-integration-01/PARENT-OBSERVATIONS-23-FAILED.json`.

Phase24thenpassed3exactEngineCTestsandthe shippingbuildonce; eachchild1passed/0failed/0ignored. It adds two successful-subtraction diagnostic counters through the existing debug log, without a new flag/sink. Newgame16699600bytes/SHA`099d51994c78e2cb60cd852de516222ff5d4f0de522471332577a85d4c7d926d`; Coreunchanged. Collector01's incorrectall-products-freshfalse expectation failed and isretained; correctedcollector02requiresunchangedCorecachedtrue/Engine-gamefalse and recheckedallgates. No native rerun. Current099hasnotbeenrunin-game, andcapture23remainsFAILED.

**Latest stock control (2026-11-12):** `python3 -B out/task-20261112/Artifacts/hud-selector-stock-02/run.py` executed the previously unrun stock variant once, with the same fixture, INI and `9b5a3dbb…` shipping binary as selector01, isolated HOME/TMP/XDG state, and `HP2_STOCK_HUD=0`. Native smoke returned0, replayed683frames and saved7PNGs; strict qualification returned1 because the identical contextual CutError block also occurred here. All488overlay entries matched manifest/checksums; executable, sources and producer files remained unchanged. Pixel comparison found zero differences at450/510/570/600/660;630has33,596changedpixels and700has276,517. Full-frame pictures, contact sheet and enlarged labels were inspected. No test/build rerun, source-runtime change, oracle-parity claim or accepted-row increment follows. Results: `hud-selector-stock-02/{RESULT.json,visual/comparison.json,stock/request.json}` under `out/task-20261112/Artifacts/`.

**Whole-HUD integration checkpoint (2026-11-12):** At this checkpoint no additional UI row was closed; the later UI-48/UI-50 acceptance below supersedes that count. The finite weapon adapter is integrated; `weapon-02` passed its exact HUD CTest and shipping build. `frontend-previous-01` exercised the real book menu with Escape and consumed all184replay frames: five full800×600RGBA frames matched the preserved pre-cutover Rust binary exactly with NativeText=False. This is previous-Rust regression evidence, not independent native parity. The toolkit/stock footer difference is retained rather than changing the existing subtitle placement policy.

**Selector control:** `selector-stock-control-02` used the actual Duel01 map and a780-frame fixture covering every capture. All seven stock/stock RGBA pairs matched exactly, including630and700. Both runs still retained the same two unfinished-cue CutErrors, so strict qualification remains failed. This establishes covered-replay repeatability, not selector acceptance or authority to alter gameplay cues. The preceding `selector-stock-control-01` mistakenly requested nonexistent Duel1 and failed before the game loop; its diagnosis is retained.

**Shared Core blocker repaired:** The actual shortcut/bookmark window exposed a production dynamic-array descriptor mismatch: the Core adapter supplied the scalar inner property where the variable kernel expected the array descriptor. `core_vm.rs` now supplies the array; `vm_variable.rs` obtains growth/stride from its inner property without caching across callbacks. `core-array-03` passed each exact `vm_variable_contracts`, `rust_core_runtime`, `ui_hud_contracts` and `canvas_compatibility_contracts` CTest, then the shipping build. The real-Core name/string-array regression passed within18Core tests, including indexed growth, overwrite, empty elements and independent returned-string ownership. Shipping SHA256 is`ac975b402c7df36c0e9be891366e71a4dc6a80f313729e95c8e81f57cf10d08f`. The first regression run SIGSEGV is preserved: LLDB identified the new test's invalid Return IntZero into a null result slot; the fixture now uses Return Nothing. Production return behavior was not weakened. Artifacts, exact commands, private environments, prior binaries and producer provenance are under`out/task-20261112/Artifacts/whole-hud-01/{core-array-02,core-array-debug-01,core-array-debug-02,core-array-03}/`. These are macOS arm64 configured CTests and isolated data-none Core metadata evidence; live retail bookmark qualification is separate.

**Canonical console-argument blocker repaired:** `shortcut-entry-01` remained in Entry and logged two BadProperty errors: `import_console_argument` passed localized flags1rather than canonical delimited flags2. `core_reflection.rs` now uses PPF_Delimited; the real native StrProperty regression checks unquoted, quoted-spaced and empty arguments, consumed length and remaining input. `console-arguments-02` passed exact `rust_core_runtime` (19passed), `console_vm_native_owner_contracts`, `ui_hud_contracts` and `canvas_compatibility_contracts`, then shipping build. Current shipping SHA256 is`dff37dd5c895fb689c50a49293e658851666aa96b6cba8f686b2c38579c869a4`. `shortcut-entry-02` then genuinely set GSTATE020, traveled from Entry to Grandstaircase_hub and rendered all seven named bookmarks without the previous Core errors. Both games exited0and consumed169frames with real loopback audio. Strict failure remains: the harness expected the wrong `.unr`-suffixed completion literal, and both variants logged the same PrepareForGoToDADA2 camera-capture CutError. The future travel expectation is corrected; the failed run is retained, not rerun to green.

**Reachability decision:** The user explicitly chose to keep UI-39–UI-41, UI-49 and UI-51’s positive-flash branch open. No profile-specific exclusions are accepted. Preserve the finite implementations and investigate genuine supported activation; do not fabricate actor/property/script state. Decision record:`whole-hud-01/reachability/authorization.json`.

**UI-48/UI-50 accepted at800×600:** `conditional-console-toggles-02`, `conditional-console-native-01` and `conditional-console-save-01` each exercised the real exec/input route with all156replay frames consumed, two screenshots, zero CutErrors and actual OpenAL loopback. All18complete decoded RGBA pairs matched exactly with stock-presence-zero, without masks/tolerance/discarded frames. Inspected pictures show ShowPos off/on/off, the original AVG/LAST SEC update and both views disabled. NativeText=False and True were independently checked in the effective private Game.ini; the native picture visibly uses native serif text. The save scenario retained separate fully reverified488-entry copy-on-write profiles and identical40-byte canonical fps.txt outputs (SHA256`6909e3407f3358ee1e8fd2a3e47f6f6d443a17d8222c3e6efb5f03ee65a89de3`). `conditional-console-toggles-01`'s actual SIGSEGV is retained: offline symbolication identified direct native argument decoding from the finite view. Both the native handler and finite TimeDemo view now reuse `engine_vm_native::entry_level_info`; no VM walker or synthetic bytecode was added. `timedemo-boundary-02` passed each exact console-native/render-console/HUD/bitmap-Canvas CTest and shipping build; current game SHA256`0b563168deee8496ce606ab8f5685158464d7df223c91a4b6df0a64b97b43279`. Exact commands, results, limits and source paths: `whole-hud-01/console-acceptance.json`. This is same-product stock comparison, not independent native parity; UI-43 and every other newly integrated row remain open.

**UI-51 remaining-source evidence:** The retained complete offline class parser now preserves actual serialized UClass flags, independently confirmed against production serialization: Engine.Weapon0x833is abstract; HWeapon/baseWand/Eightball0x32are nonabstract. Every class export parsed completely. These flags are not the export object flags formerly retained in classes.json. `weapon-boundary/serialized-class-flags.json` retains package identities/default tags; this closes that static uncertainty, not the still-open authored first-person receiver or positive MFTexture/flash activation condition.

**Final normal-flow smoke under the approved gate:** final binary `0b563168deee8496ce606ab8f5685158464d7df223c91a4b6df0a64b97b43279` ran `python3 -B out/task-20261112/Artifacts/whole-hud-01/capture_replay_modes.py replay-delivery-01` and `python3 -B out/task-20261112/Artifacts/whole-hud-01/capture_components.py frontend-delivery-01 Startup out/task-20261112/Artifacts/whole-hud-01/frontend-capture-scenario.txt 190 10,30,50,90,140 --frontend --baseline-game out/task-20261112/Artifacts/whole-hud-01/before/out/macos-arm64/hp2_game_rust`. All8actual games exited0 with zero CutErrors. Six direct recording/replay/inactive cases used actual OpenAL loopback and private homes; each replay consumed its genuinely recorded file. All9record/active-replay/inactive RGBA pairs match; post-EOF78retains66changed pixels and is not an exact-match pass. The book/menu scenario consumed all184inputs and all5complete RGBA frames match the preserved previous Rust product. Recording/replay/inactive, positive/off FPS, book and post-EOF pictures were inspected. This is macOS arm64, prepared retail, 800×600, NativeText=False, not independent native parity or exhaustive UI-43 acceptance. NativeText=True ShowPos/TimeDemo evidence remains separately recorded above.



Future updates must name: **row closed; what runs in the actual game; evidence; next row; concrete blocker if any**. Do not substitute review count, artifact count, binary count, or a guessed percentage for completed UI rows.
