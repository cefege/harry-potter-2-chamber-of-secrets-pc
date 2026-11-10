# Stock menu layout (Sound/Video and Controls pages)

Retail's pages are patched at run time by observers (`stock_graphics_menu`, `stock_input_menu`,
`stock_display_control`, `stock_render_scale_control`, `stock_fps_menu`); no retail package is edited.
Coordinates are page units on a 640x480 root. Use `HP2_LAYOUT_DUMP=1` to log every child's geometry,
and the replay harness (`Tests/InputScriptSmoke.py`, `HP2_CAPTURE_FRAMES`) to take screenshots.

## Sound/Video page
Two columns (x 30 and 326, 280 wide), then an "Advanced" block below them:

| Video (x 30) | Audio (x 326) |
| --- | --- |
| Logical res., Textures, Object Detail, Brightness, 3D Scale | Music, Sound, Display, Show FPS |

Advanced (y 282..): Frame limit, VSync, Anti-alias, Anisotropy | UI scale, Vertical FOV, Flashes.
One stock combo per setting (the old "Advanced"/"Value" selector pair is gone). Lists with more than
four values show three at a time plus "More values...". Colour depth is hidden (always 32-bit).
Native text is not a menu choice: it is on by default and falls back to bitmap text by itself when
no text backend starts. `[SDLDrv.SDLClient] NativeText=False` still forces bitmap text for
compatibility tests.

Findings that explain earlier oddities:
* A retail combo's value box is a fixed 180-unit bitmap drawn at `WinWidth - EditBoxWidth`; a narrower
  `EditBoxWidth` crops its right cap.
* Retail combos are `Align = TA_Right`, which makes `EditAreaDrawX` 0, so their edit box, drop-down
  button (as wide as the box) and list were laid out from the control's left edge instead of under the
  art. That is why a click sometimes opened the list and sometimes not, and why lists opened away
  from the pointer. Combos are now `TA_Left`, and `popup_after_paint` keeps every list under its box.
* UWindow hit-tests by geometry only, so a hidden child still takes clicks; remove it from the window
  list (`HideWindow`) instead of clearing `bWindowVisible`.

## Controls page
* "Exact value": an editable stock combo under the Mouse Speed slider. It follows the slider every
  tick (unless being typed in) and applies and saves valid text between 0.2 and 10.
* "Mouse Smoothing": toggles `[Engine.PlayerPawn] bMaxMouseSmoothing` (the script's own smoothing
  option; retail's page had no control for it). The script's base two-sample averaging is not
  optional.
* Slider changes are now saved to User.ini as well.
* The Quidditch/duelling blocks move down 22 units; the "Cycle Current Spell" label no longer clips.
* Mouse acceleration: the scripts have none. The OS pointer curve reaches the game through SDL's
  relative mouse mode (not measured here).

`HP2_INPUT_SELFTEST=<number>` feeds the box a value once (replays cannot type text);
`Tests/RetailStockInputControlsSmoke.py` uses it. Retail data and a display are required.
