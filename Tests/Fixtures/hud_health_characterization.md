# Health HUD characterization (source and focused bytecode, not runtime evidence)

Reference: `HarryPotter2/Unreal/HGame/Classes/Menu/StatusItemHealth.uc`,
`DrawItem`. This is a test design fixture, not a recorded replay or accepted golden.
The full retail function and the actual engine need further characterization before
a replacement can be accepted. The focused package observation below verifies one
arithmetic expression, not the whole draw function.

- The draw function ignores the supplied `nCurrX` and `nCurrY`. It derives integer
  coordinates from `fScaleFactor`, with three icons per row, then a second row.
- Icon count is `nCurrCountPotential / nUnitsPerIcon`, rounded up for a positive
  remainder. Each icon uses up to `nUnitsPerIcon` of `nCount`. Empty icons still
  receive a black background and border. The orange tile is omitted when its fill
  ratio is not positive.
- For `textureHudIcon.VSize = 128`, the unscaled orange source segment has height
  `fill * 115 + 11` and begins at `117 - fill * 115`. Thus full fill uses
  `(start, height) = (2, 126)` and half fill uses `(59.5, 68.5)`.
- For each icon, draw black in style 3, then border in the incoming style, then
  the filled segment in that style. During a non-`NormalDisplay` state, call the
  state-dispatched `GetHealthDrawColor` for each icon whose inclusive index interval
  contains `float(nCount) / float(nUnitsPerIcon)`. A boundary can call it twice.
- The script restores `DrawColor` only. Canvas position and other native draw side
  effects require measurement; the common `CanvasState` restore is not equivalent
  without that evidence. Script timers, state changes, and pickup logic remain stock.

## Focused package observation

A private inspector using the current Rust `OwnedPackage` and `ExportArchive` APIs
observed native 161 in `HGame.StatusItemHealth.DrawItem`, package version 79,
class export 291, function export 10285. At package offset `0x135568`:

```text
a1 00 50 22 45 ab 40 01 74 3c 00 59 0d 16 16
```

The export table resolves the operands to local `nX`, instance `nActualIconW`, and
local `fScaleFactor`. The bytecode performs `IntToFloat(nActualIconW)`, float
multiplication by `fScaleFactor`, then `FloatToInt` before native 161's integer
compound addition. It does not use `textureHudIcon.USize` for this advance.

`System/HGame.u` SHA-256:
`cd0c8b849c8fab45290849f8ac14469536d468785f45aac74dd0241fba871ce3`.
This matches the disposable overlay's checksum list; it is not original-media
authentication. Commands, tool/library identities, and the bounded export dump
are retained under `out/task-20261111/Artifacts/hud-retail-bytecode/`.
Callback, tint, and other arithmetic order were not decoded by this observation.

## Required runtime evidence

Required runtime cases: zero, half and full fill; exact icon boundaries; more than
three icons; non-integer scale; damage and healing hold/fade; original canvas
properties and cursor effects; callback sequence and failures; stock fallback.
Capture the same replay frames with and without `HP2_STOCK_HUD`, including bitmap
text mode. Record executable identity, data/index provenance and artifact paths.
