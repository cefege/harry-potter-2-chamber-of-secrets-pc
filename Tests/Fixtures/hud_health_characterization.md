# Health HUD characterization (source-derived, not runtime evidence)

Reference: `HarryPotter2/Unreal/HGame/Classes/Menu/StatusItemHealth.uc`,
`DrawItem`. This is a test design fixture, not a recorded replay or accepted golden.
The retail package and the actual engine need independent characterization before a
replacement can be accepted.

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

Required runtime cases: zero, half and full fill; exact icon boundaries; more than
three icons; non-integer scale; damage and healing hold/fade; original canvas
properties and cursor effects; callback sequence and failures; stock fallback.
Capture the same replay frames with and without `HP2_STOCK_HUD`, including bitmap
text mode. Record executable identity, data/index provenance and artifact paths.
