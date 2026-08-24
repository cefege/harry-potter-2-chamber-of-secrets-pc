# RNG & tick divergence — Phase 3 acceptance record

Status: accepted (Phase 3 gate G3, 2026-08-24).
Owner: Phase 3 headless simulation (`crates/hp-engine`, `crates/hp2rs`).

This is the legislated behavior-change record required by the plan's
standing policy 4: the Rust engine's determinism policy intentionally
diverges from the C++ engine. Nothing here was bundled silently.

## 1. What diverges from C++

| Aspect | C++ engine (frozen oracle) | hp2rs (this record) |
|---|---|---|
| Randomness | unseeded libc `rand()` process-global (`UnAnsi.cpp`), consumed over variable-delta ticks — documented source of cutscene non-determinism | ONE `rand_chacha::ChaCha8Rng` per process |
| Seed | none (time/ASLR-derived) | `--rng-seed=<u64>`, default literal `0x48503200` ("HP2\0") |
| Tick delta | measured wall clock per frame, capped by `GetMaxTickRate` / `FrameRateLimit` | `--fixed-dt=<secs>` override; otherwise capped cadence `1/max_tick_rate` (30 editor mode, else `[Engine.GameEngine] FrameRateLimit`, else nominal 60 Hz) |
| Script events | full UnrealScript runtime incl. states, foreach iterators, latent actions | subset (see §4); deferred constructs are loud and counted |

Consequence: two hp2rs runs with the same seed and `--fixed-dt` are
bit-reproducible; the C++ binary is not reproducible run-to-run by
construction. Replay/determinism gates that are impossible in C++ are a
design property here.

## 2. Determinism evidence

Double-run of the smoke pipeline (same seed `0x48503200`,
`--fixed-dt=1/30`, map `Entry.unr`, scripted input + idle fill to 120
ticks):

- `cargo test -p hp-engine --test phase3_smoke determinism_double_run_identical`
  asserts equal run summaries AND equal whole-arena SHA-256 snapshot
  hashes (`hp_uobject::snapshot::snapshot_world`) across two independent
  in-process runs. Green.
- Binary-level check: `target/release/hp2rs … -testticks=120` twice with
  identical argv produces byte-identical stdout/stderr (the only
  non-deterministic output, `frame_ms=` timing lines, is gated behind
  env `HP2_FRAME_TIMING=1` and absent by default). The `<HP2_RES>
  rng_probe=<hex>` line printed once per run is seed-derived and
  identical across runs.

## 3. G3 gate evidence (frozen harness protocol)

All three gates pass `"passed": true` with zero failure markers:

```
python3 Build/game_test.py run --map Maps/PrivetDr.unr  --data-root HarryPotter2/Unreal \
  --renderer xopengl --ticks 120 --timeout 120 --log /tmp/g3_PrivetDr.log \
  --app dist/macos-arm64/HarryPotter2.app --engine-bin target/release/hp2rs
python3 Build/game_test.py run --map Maps/Entry.unr      … (same flags)
python3 Build/game_test.py run --map Maps/Ch2Skurge.unr  … (same flags)
```

Summaries (from `/tmp/g3_*.json`): all three report `passed=true`,
`exit_status=0`, `timed_out=false`, zero `failure_markers`,
`launch_error=null`. Note: exact argparse spelling requires `--map
Maps/<Name>.unr` (the harness resolves selectors against the recursive
map listing; bare names are rejected) and the map token handed to the
engine is the System-relative Windows-style path (`..\Maps\<Name>.unr`),
which `hp-engine::level::resolve_map_path` normalizes.

## 4. Accepted script-fidelity deferral (`engine.script_event_deferred`)

The Phase-2 VM executes real UnrealScript bodies but deliberately defers
some constructs. Disassembling shipped bytecode showed faithful `foreach`
support would require re-deriving the code-stream ABI (locals addressing,
call dispatch into bodies, out-parameter binding), which is out of Phase-3
scope. Per the plan's fallback policy ("explicitly accepted reason-coded
deferral — never a silent stub"), Main accepted this amendment:

- Tractable control flow WAS implemented for real: `EX_Switch`,
  `EX_Case` (C++ `execSwitch`/`execCase` semantics incl. MAXWORD default
  and fallthrough), `EX_LabelTable` parsing, `EX_GotoLabel` (function
  context consumes the label and continues, matching `UObject::GotoLabel`
  outside a state), and numbered-native dispatch for every opcode ≥
  `EX_FirstNative` (parameter evaluation + registered-slot call).
- Any actor event that hits a deferred construct fails LOUDLY to stderr
  as `[engine.script_event_deferred] <forwarded reason>`, is counted in
  `RunCounters.script_deferrals` and the run summary line, and its
  `(class, event)` site is remembered so later frames skip it
  deterministically. First occurrence per site prints; repeats only bump
  the counter.
- Deferral classes: `vm.token_unsupported`, `vm.unknown_token_deferrable`
  (unknown opcodes inside fork-undocumented gaps 0x03/0x35/0x5B–0x6F),
  `vm.lvalue_unsupported`, `vm.code_truncated`, `native.*`. Everything
  else — genuinely unknown opcodes outside gaps, bad data — aborts the
  run nonzero.
- Observed deferral census at G3 (120 ticks, release build): Entry = 0;
  PrivetDr ≈ 47 sites (`EX_Iterator` on InterpolationPoint BeginPlay,
  deferred native bodies on pawns/Mover Tick); Ch2Skurge ≈ 76 sites
  (same families). No hard failures.

Open fidelity item for a post-G3 phase (recorded here because it blocks
faithful `foreach`): code-stream operand encoding vs the package compact
index. Evidence dossier: `InterpolationPoint.BeginPlay` begins
`1c 48 05 16 2f 61 …` where the iterator factory call surfaces opcode
byte `0x61`, which the frozen `UnStack.h` does not define (gaps:
0x03/0x35/0x5B–0x5F) under every candidate start anchor we scored
(±4 bytes) and every operand model tried (package CI, raw i32, LEB128);
a budget-capped model-comparison spike over 5531 loaded function bodies
did not separate the models cleanly either. The spike script was removed
at cleanup; the probe methodology lives in this paragraph.

## 5. Flags this record commits to

- `--rng-seed=<u64>` (hex with 0x prefix or decimal), default
  `0x48503200`.
- `--fixed-dt=<secs>` (finite, positive) freezes the tick cadence.
- `HP2_FRAME_TIMING=1` enables per-tick `<HP2_RES> frame_ms=` lines;
  unset keeps output fully deterministic so log hashing works.

## 6. Fidelity follow-up reference (added 2026-08-24)

Community resource for the deferred-construct work:
https://github.com/metallicafan212/HP2UScriptDecompile/ — working decompiles
of the shipped obfuscated UScript (HGame classes incl. InterpolationPoint,
Harry pawn, bosses). Use as READ-ONLY behavioral reference for the faithful
foreach/code-stream ABI rework; never vendored. Its decompiles recompile
through ucc make, reinforcing that the shipped token stream follows stock
compiler conventions (the presumed '0x61 fork opcode' remains under
investigation as a probable anchor artifact).
