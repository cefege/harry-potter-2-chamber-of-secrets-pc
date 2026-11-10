# Menu rewrite: engine-drawn, reactive Rust UI

**Decision (owner, this session):** replace *every* stock UnrealScript menu with menus written in
Rust, reactive and HTML-like, that look the same. Built from scratch (no webview, no large UI
dependency), drawn on the engine's own canvas with the retail art and fonts. AGENTS.md was updated
to say so. Until a screen is replaced its stock page and the `stock_*` observers stay supported.

## Status

| Piece | State |
| --- | --- |
| `hp2-ui` toolkit (signals, flex layout, widgets, input, draw list) | **Done, 15 unit tests** (`ui_toolkit_contracts`) |
| Engine adapter (fonts, art, canvas drawing, input routing, menu manager) | Not started |
| Any screen on the new toolkit | None yet; no screen is replaced |

Nothing the player sees has changed yet. The adapter is the next step and the risky one.

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
