# Menu rewrite: engine-drawn, reactive Rust UI

**Decision (owner, this session):** replace *every* stock UnrealScript menu with menus written in
Rust, reactive and HTML-like, that look the same. Built from scratch (no webview, no large UI
dependency), drawn on the engine's own canvas with the retail art and fonts. AGENTS.md was updated
to say so. Until a screen is replaced its stock page and the `stock_*` observers stay supported.

## Status

| Piece | State |
| --- | --- |
| `hp2-ui` toolkit (signals, flex layout, widgets, input, draw list, skins) | Done, 15 unit tests (`ui_toolkit_contracts`) |
| Engine adapter: `ui_host` (canvas drawing through the page's own script draw calls, text measured by the page's `TextSize`, retail textures), `ui_vm` (typed script calls) | Done |
| **Sound/Video page** (`ui_pages`) | **Replaced.** Drawn and handled by `hp2-ui`; verified by replay screenshots and by settings written to the ini |
| Controls page, pause book pages, save/load, main menu, message boxes | Still stock (with the `stock_*` observers) |

`HP2_STOCK_MENUS=1` brings back the stock page. `HP2_UI_DEBUG=1` logs input and applied commands.

### How a replaced page works
The stock page is still created by the game, so its script state, the confirmation dialogs and
the `stock_*` setting observers keep working. The Rust view hides the page's stock controls (the
Back button stays), mirrors their values into signals every paint, and queues the player's changes.
On the next engine tick each change is written into the stock control and raised with the
control's own `Notify(DE_Change)`, so the stock handlers and the observers run exactly as for a
click. (Calling the page's `Notify` from Rust does not reach the script-call hooks; the control's
script must forward it.)

Evidence (retail data, macOS arm64, wgpu, replay `ui_click`/`ui_click2`):
Frame limit 144 -> 30 (`FrameRateLimit=30`), Show FPS on (`ShowFPS=True`), Music volume 0,
Object detail Very Low, Textures Low were all applied and saved by clicks on the Rust page; the
screenshot matches the stock layout.

## The toolkit (`crates/hp2-ui`)

* `Store` + `Signal<T>`: state; setting a different value bumps a revision.
* Views are functions returning a `Node` tree: `col`, `row`, `spacer`, `label`, `button`, `checkbox`,
  `slider`, `select`, `number_input`; chain `.class()`, `.key()`, `.gap()`, `.padding()`, `.width(Size)`,
  `.align()`, `.justify()`, `.at(x, y)`, `.on_click()`. A `Stylesheet` maps class names to `Style`.
* `Ui` rebuilds the tree when the revision changed, lays it out (rows, columns, gap, padding, fixed /
  auto / grow sizes, alignment, justify, absolute children), routes pointer, key and text input
  (hover, press-and-release clicks, slider drag, drop-down popups, Tab focus, number editing that
  applies valid keystrokes live and clamps on commit) and paints a backend-neutral `DrawCmd` list.
* A `Skin` maps widget parts (box, list, slider track/thumb, checkbox states, button) to texture
  names, so the toolkit knows nothing about the engine.

```rust
let store = Store::new();
let speed = store.signal(3.0_f32);
let view = { let speed = speed.clone(); move || {
    let s = speed.clone();
    col().gap(6.).child(label("Mouse Speed").class("caption"))
        .child(slider(0.2, 10., 0.2, speed.get(), { let s = s.clone(); move |v| s.set(v) }).key("speed"))
        .child(number_input(speed.get(), 0.2, 10., 2, move |v| s.set(v)).key("speed-box"))
}};
let mut ui = Ui::new(store, view, metrics).with_stylesheet(sheet).with_skin(skin);
// each frame: ui.pointer_move/down/up, ui.key, ui.text, then
let commands = ui.draw();
```

## Plan for the engine adapter and screens

1. **Adapter (`hp2-engine`, new `ui_host`)**: `TextMetrics` from the native text layout (the same
   measure the canvas uses); `Skin` names resolved to `UTexture` objects (`HGame.FE*Texture`);
   `DrawCmd` replayed through `canvas_owner::native_draw_tile` / `draw_text`; clipping via the canvas
   clip; pointer, key and SDL text input routed from the platform pump while a Rust menu is up; the
   game pauses and the stock menu is not shown. A `HP2_LAYOUT_DUMP`-style screenshot test through the
   replay harness is the acceptance check for looks.
2. **Options pages first** (Sound/Video, Controls): the screens already reworked in
   `Docs/STOCK_MENU_LAYOUT.md`, reproduced in the toolkit with the same art. Retire their observers.
3. **Pause book and hub**, save/load, folio, house points, spells, cards, credits.
4. **Main menu** (`Startup.unr`): the 3D scene stays; only the controls move.
5. Delete the stock-page observers and the `stock_*` modules when nothing uses them; keep the
   UnrealScript packages untouched (retail data is never edited).

Unknown until step 1 is built: how many stock screens depend on script state that Rust must
reproduce (the book uses a 3D model and sounds; some pages call script functions), and the exact
cost of drawing text through the canvas each frame. Each screen needs its own retail screenshot
comparison before the stock version is retired.
