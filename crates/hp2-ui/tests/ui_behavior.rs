//! Behavior of the toolkit through its public API only.
use std::{cell::Cell, rc::Rc};

use hp2_ui::{
    Align, Color, DrawCmd, FixedMetrics, Justify, Key, PointerButton, Rect, Size, Skin, SkinPart,
    Store, Style, Stylesheet, Ui, button, checkbox, col, label, number_input, row, select, slider,
    spacer,
};

const LEFT: PointerButton = PointerButton::Left;

#[test]
fn image_keeps_absolute_geometry_and_does_not_take_keyboard_focus() {
    use hp2_ui::{Fit, TextureRef, image};
    let mut ui = Ui::new(
        Store::new(),
        || {
            col().children([
                image(TextureRef("spell".to_owned()), Fit::Stretch)
                    .key("art")
                    .at(3., 175.)
                    .width(Size::Px(64.))
                    .height(Size::Px(32.)),
                button("Action").key("action"),
            ])
        },
        FixedMetrics::default(),
    );
    ui.resize(640., 480.);
    assert!(ui.draw().contains(&DrawCmd::Image {
        texture: TextureRef("spell".to_owned()),
        rect: Rect::new(3., 175., 64., 32.),
        fit: Fit::Stretch,
        color: Color::WHITE,
        blend: hp2_ui::BlendMode::Inherit,
    }));
    assert_eq!(ui.text_requests().len(), 1, "art is not a text request");
    ui.key(Key::Tab);
    assert_eq!(ui.focused(), Some("action"));
}

#[test]
fn image_paint_tracks_resolved_style_changes_without_tinting_widget_art() {
    use hp2_ui::{BlendMode, Fit, TextureRef, image};
    let store = Store::new();
    let paint = store.signal((Color(12, 34, 56, 78), BlendMode::Translucent));
    let observed = paint.clone();
    let mut ui = Ui::new(
        store,
        move || {
            let (color, blend) = observed.get();
            col().child(
                image(TextureRef("meter".into()), Fit::Region(2., 3., 11., 13.))
                    .class("image")
                    .color(color)
                    .blend(blend)
                    .at(-7., 9.)
                    .width(Size::Px(22.))
                    .height(Size::Px(26.)),
            ).child(button("label").color(color))
        },
        FixedMetrics::default(),
    ).with_stylesheet(Stylesheet::new().with("image", Style {
        color: Color(1, 2, 3, 4),
        blend: BlendMode::Masked,
        ..Style::default()
    })).with_skin(Skin::new().with(SkinPart::ButtonIdle, "button"));
    ui.resize(640., 480.);
    for (color, blend) in [
        (Color(12, 34, 56, 78), BlendMode::Translucent),
        (Color(91, 82, 73, 64), BlendMode::Modulated),
    ] {
        paint.set((color, blend));
        let commands = ui.draw();
        assert_eq!(commands[0], DrawCmd::Image {
            texture: TextureRef("meter".into()),
            rect: Rect::new(-7., 9., 22., 26.),
            fit: Fit::Region(2., 3., 11., 13.),
            color,
            blend,
        });
        assert!(commands.iter().any(|command| matches!(command,
            DrawCmd::Image { texture, color: Color::WHITE, blend: BlendMode::Inherit, .. }
                if texture.0 == "button"
        )), "text color must not change existing menu artwork");
    }
}

fn click(ui: &mut Ui, x: f32, y: f32) {
    ui.pointer_move(x, y);
    ui.pointer_down(x, y, LEFT);
    ui.pointer_up(x, y, LEFT);
}

fn center(ui: &Ui, key: &str) -> (f32, f32) {
    let r = ui.rect_of(key).unwrap_or_else(|| panic!("no node {key}"));
    (r.x + r.w / 2., r.y + r.h / 2.)
}

#[test]
fn row_and_column_layout_with_gap_padding_grow_and_alignment() {
    let store = Store::new();
    let view = || {
        col()
            .key("page")
            .gap(10.)
            .padding(hp2_ui::Edges::all(8.))
            .child(label("Title").key("title"))
            .child(
                row()
                    .key("bar")
                    .gap(4.)
                    .align(Align::Center)
                    .height(Size::Px(30.))
                    .child(label("ab").key("a"))
                    .child(spacer().key("fill"))
                    .child(label("cd").key("b")),
            )
    };
    let mut ui = Ui::new(store, view, FixedMetrics::default());
    ui.resize(200., 100.);
    // Column: padding 8, title 5 chars * 7 = 35 wide, 14 high, then a 10 gap.
    assert_eq!(ui.rect_of("title"), Some(Rect::new(8., 8., 35., 14.)));
    let bar = ui.rect_of("bar").unwrap();
    assert_eq!((bar.x, bar.y, bar.h), (8., 32., 30.));
    // Row: "ab" at the left, "cd" pushed to the right edge by the growing spacer, both centred vertically.
    let (a, b) = (ui.rect_of("a").unwrap(), ui.rect_of("b").unwrap());
    assert_eq!((a.x, a.y), (8., 32. + (30. - 14.) / 2.));
    assert!(
        (b.right() - bar.right()).abs() < 0.01,
        "right label must end at the bar's right edge: {b:?} {bar:?}"
    );
}

#[test]
fn justify_space_between_distributes_the_leftover() {
    let store = Store::new();
    let view = || {
        row()
            .key("r")
            .justify(Justify::SpaceBetween)
            .child(label("a").key("a"))
            .child(label("b").key("b"))
            .child(label("c").key("c"))
    };
    let mut ui = Ui::new(store, view, FixedMetrics::default());
    ui.resize(100., 20.);
    let xs: Vec<f32> = ["a", "b", "c"]
        .iter()
        .map(|k| ui.rect_of(k).unwrap().x)
        .collect();
    assert_eq!(xs, vec![0., 46.5, 93.]);
}

#[test]
fn views_rebuild_only_when_a_signal_changed() {
    let store = Store::new();
    let count = store.signal(0);
    let builds = Rc::new(Cell::new(0));
    let (c, b) = (count.clone(), builds.clone());
    let view = move || {
        b.set(b.get() + 1);
        let c = c.clone();
        col()
            .child(label(&format!("clicked {}", c.get())).key("text"))
            .child(
                button("Add")
                    .key("add")
                    .on_click(move || c.update(|n| *n += 1)),
            )
    };
    let mut ui = Ui::new(store, view, FixedMetrics::default());
    assert_eq!(builds.get(), 1);
    assert!(!ui.update());
    let (x, y) = center(&ui, "add");
    click(&mut ui, x, y);
    click(&mut ui, x, y);
    assert_eq!(count.get(), 2);
    assert_eq!(
        builds.get(),
        3,
        "one rebuild per change, none for idle updates"
    );
    let texts: Vec<String> = ui
        .draw()
        .into_iter()
        .filter_map(|c| {
            if let DrawCmd::Text { text, .. } = c {
                Some(text)
            } else {
                None
            }
        })
        .collect();
    assert!(texts.contains(&"clicked 2".to_owned()), "{texts:?}");
}

#[test]
fn a_button_fires_only_when_released_over_the_pressed_button() {
    let store = Store::new();
    let hits = store.signal(0);
    let h = hits.clone();
    let view = move || {
        let h = h.clone();
        row()
            .gap(20.)
            .child(
                button("One")
                    .key("one")
                    .on_click(move || h.update(|n| *n += 1)),
            )
            .child(button("Two").key("two"))
    };
    let mut ui = Ui::new(store, view, FixedMetrics::default());
    let (one, two) = (center(&ui, "one"), center(&ui, "two"));
    ui.pointer_down(one.0, one.1, LEFT);
    ui.pointer_up(two.0, two.1, LEFT);
    assert_eq!(hits.get(), 0, "released elsewhere: no click");
    click(&mut ui, one.0, one.1);
    assert_eq!(hits.get(), 1);
}

#[test]
fn checkbox_toggles_by_click_and_by_space() {
    let store = Store::new();
    let on = store.signal(false);
    let o = on.clone();
    let view = move || {
        let o = o.clone();
        let current = o.get();
        col().child(checkbox("Smoothing", current, move |v| o.set(v)).key("smooth"))
    };
    let mut ui = Ui::new(store, view, FixedMetrics::default());
    let (x, y) = center(&ui, "smooth");
    click(&mut ui, x, y);
    assert!(on.get());
    ui.key(Key::Space);
    assert!(
        !on.get(),
        "the clicked checkbox keeps focus, so Space toggles it again"
    );
}

#[test]
fn slider_follows_the_pointer_snaps_to_its_step_and_the_arrow_keys() {
    let store = Store::new();
    let value = store.signal(5.0_f32);
    let v = value.clone();
    let view = move || {
        let v = v.clone();
        let current = v.get();
        col().child(slider(0., 10., 1., current, move |n| v.set(n)).key("speed"))
    };
    let mut ui = Ui::new(store, view, FixedMetrics::default());
    let r = ui.rect_of("speed").unwrap();
    ui.pointer_move(r.x + r.w - 1., r.y + 5.);
    ui.pointer_down(r.x + r.w - 1., r.y + 5., LEFT);
    assert_eq!(value.get(), 10.);
    ui.pointer_move(r.x - 50., r.y + 5.);
    assert_eq!(value.get(), 0., "dragging past the end clamps");
    ui.pointer_up(r.x - 50., r.y + 5., LEFT);
    ui.pointer_down(r.x + r.w / 2., r.y + 5., LEFT);
    ui.pointer_up(r.x + r.w / 2., r.y + 5., LEFT);
    assert_eq!(value.get(), 5., "the middle snaps to a whole step");
    ui.key(Key::Right);
    ui.key(Key::Right);
    ui.key(Key::Left);
    assert_eq!(value.get(), 6.);
}

#[test]
fn select_opens_picks_a_row_cycles_by_key_and_closes_on_escape() {
    let store = Store::new();
    let choice = store.signal(1_usize);
    let c = choice.clone();
    let view = move || {
        let c = c.clone();
        let current = c.get();
        col()
            .child(
                select(
                    &["Unlimited", "30 FPS", "60 FPS", "144 FPS"],
                    current,
                    move |i| c.set(i),
                )
                .key("fps"),
            )
            .child(button("Below").key("below"))
    };
    let mut ui = Ui::new(store, view, FixedMetrics::default());
    let (x, y) = center(&ui, "fps");
    click(&mut ui, x, y);
    assert_eq!(ui.open_select(), Some("fps"));
    let r = ui.rect_of("fps").unwrap();
    // Third row of the popup: 3 px top inset, 24 px rows.
    click(&mut ui, r.x + 20., r.bottom() + 3. + 24. * 3. + 12.);
    assert_eq!((choice.get(), ui.open_select()), (3, None));
    ui.key(Key::Up);
    assert_eq!(choice.get(), 2, "arrow keys cycle a closed select directly");
    click(&mut ui, x, y);
    ui.key(Key::Escape);
    assert_eq!((ui.open_select(), choice.get()), (None, 2));
    // A click outside an open popup only closes it.
    click(&mut ui, x, y);
    let below = center(&ui, "below");
    click(&mut ui, below.0, below.1);
    assert_eq!(ui.open_select(), None);
}

#[test]
fn number_input_applies_each_valid_keystroke_clamps_on_commit_and_reverts_junk() {
    let store = Store::new();
    let speed = store.signal(3.0_f32);
    let s = speed.clone();
    let view = move || {
        let s = s.clone();
        let current = s.get();
        col()
            .child(number_input(current, 0.2, 10., 1, move |n| s.set(n)).key("speed"))
            .child(button("Elsewhere").key("else"))
    };
    let mut ui = Ui::new(store, view, FixedMetrics::default());
    let (x, y) = center(&ui, "speed");
    click(&mut ui, x, y);
    assert!(ui.is_editing());
    // The first character replaces the old text.
    ui.text("7");
    assert_eq!(speed.get(), 7.);
    ui.text(".5");
    assert_eq!(speed.get(), 7.5);
    ui.text("x1");
    assert_eq!(
        speed.get(),
        7.5,
        "letters are ignored and a second decimal is refused (one decimal place)"
    );
    ui.key(Key::Backspace);
    ui.key(Key::Backspace);
    assert_eq!(
        speed.get(),
        7.,
        "text '7' parses again after deleting the fraction"
    );
    ui.text("5");
    ui.text("0");
    // 750 is out of range, so the live value stays and the commit clamps it.
    assert_eq!(speed.get(), 7.);
    ui.key(Key::Enter);
    assert!(!ui.is_editing());
    assert_eq!(
        speed.get(),
        10.,
        "Enter applies the clamped number (750 -> 10)"
    );
}

#[test]
fn number_input_clamps_out_of_range_text_when_editing_ends() {
    let store = Store::new();
    let speed = store.signal(3.0_f32);
    let s = speed.clone();
    let view = move || {
        let s = s.clone();
        let current = s.get();
        col()
            .child(number_input(current, 0.2, 10., 1, move |n| s.set(n)).key("speed"))
            .child(button("Elsewhere").key("else"))
    };
    let mut ui = Ui::new(store, view, FixedMetrics::default());
    let (x, y) = center(&ui, "speed");
    click(&mut ui, x, y);
    ui.text("99");
    assert_eq!(speed.get(), 3., "out of range is not applied live");
    let other = center(&ui, "else");
    click(&mut ui, other.0, other.1);
    assert_eq!(speed.get(), 10., "leaving the box clamps and applies");
    click(&mut ui, x, y);
    ui.text(".");
    ui.text("1");
    assert_eq!(speed.get(), 10., "0.1 is below the minimum: not applied");
    ui.key(Key::Escape);
    assert_eq!(speed.get(), 10.);
    assert!(!ui.is_editing());
}

#[test]
fn tab_walks_focus_in_order_and_skips_disabled_widgets() {
    let store = Store::new();
    let view = || {
        col()
            .child(button("a").key("a"))
            .child(button("b").key("b").disabled(true))
            .child(checkbox("c", false, |_| {}).key("c"))
            .child(label("not focusable").key("l"))
    };
    let mut ui = Ui::new(store, view, FixedMetrics::default());
    ui.key(Key::Tab);
    assert_eq!(ui.focused(), Some("a"));
    ui.key(Key::Tab);
    assert_eq!(ui.focused(), Some("c"));
    ui.key(Key::Tab);
    assert_eq!(ui.focused(), Some("a"));
    ui.key(Key::BackTab);
    assert_eq!(ui.focused(), Some("c"));
}

#[test]
fn classes_style_nodes_and_builder_calls_override_them() {
    let store = Store::new();
    let sheet = Stylesheet::new()
        .with(
            "heading",
            Style {
                color: Color(255, 255, 255, 255),
                padding: hp2_ui::Edges::all(4.),
                ..Style::default()
            },
        )
        .with(
            "green",
            Style {
                color: Color(0, 200, 0, 255),
                ..Style::default()
            },
        );
    let view = || {
        col().child(label("Video").class("heading").key("h")).child(
            label("Audio")
                .class("green")
                .color(Color(1, 2, 3, 255))
                .key("g"),
        )
    };
    let mut ui = Ui::new(store, view, FixedMetrics::default()).with_stylesheet(sheet);
    let colors: Vec<Color> = ui
        .draw()
        .into_iter()
        .filter_map(|c| {
            if let DrawCmd::Text { color, .. } = c {
                Some(color)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(colors, vec![Color(255, 255, 255, 255), Color(1, 2, 3, 255)]);
    assert_eq!(
        ui.rect_of("h").unwrap().h,
        14. + 8.,
        "padding from the class counts in the box"
    );
}

#[test]
fn a_skin_replaces_fallback_rectangles_and_popups_paint_last() {
    let store = Store::new();
    let view = || {
        col()
            .child(select(&["a", "b"], 0, |_| {}).key("s"))
            .child(button("below").key("b"))
    };
    let skin = Skin::new()
        .with(SkinPart::BoxIdle, "HGame.FEComboIdleTexture")
        .with(SkinPart::ListBackground, "HGame.FEComboListSmall");
    let mut ui = Ui::new(store, view, FixedMetrics::default()).with_skin(skin);
    let before = ui.draw();
    assert!(
        matches!(before.first(), Some(DrawCmd::PushClip(_))),
        "widget art is clipped to the widget"
    );
    let first_image = before.iter().find(|c| matches!(c, DrawCmd::Image { .. }));
    assert!(
        matches!(first_image, Some(DrawCmd::Image { texture, .. }) if texture.0 == "HGame.FEComboIdleTexture")
    );
    let (x, y) = center(&ui, "s");
    click(&mut ui, x, y);
    let open = ui.draw();
    let popup = open.iter().position(|c| matches!(c, DrawCmd::Image { texture, .. } if texture.0 == "HGame.FEComboListSmall")).expect("popup background");
    let below_button = open
        .iter()
        .rposition(|c| matches!(c, DrawCmd::Text { text, .. } if text == "below"))
        .expect("button text");
    assert!(
        popup > below_button,
        "the popup must draw over later siblings"
    );
}

#[test]
fn state_for_removed_widgets_is_dropped_on_rebuild() {
    let store = Store::new();
    let show = store.signal(true);
    let s = show.clone();
    let view = move || {
        let mut page = col().child(button("always").key("always"));
        if s.get() {
            page = page.child(number_input(1., 0., 5., 0, |_| {}).key("temp"));
        }
        page
    };
    let mut ui = Ui::new(store, view, FixedMetrics::default());
    let (x, y) = center(&ui, "temp");
    click(&mut ui, x, y);
    assert!(ui.is_editing() && ui.focused() == Some("temp"));
    show.set(false);
    ui.update();
    assert!(!ui.is_editing());
    assert_eq!(ui.focused(), None);
}

#[test]
fn the_focus_ring_shows_only_after_keyboard_navigation() {
    let store = Store::new();
    let view = || {
        col()
            .child(button("a").key("a"))
            .child(button("b").key("b"))
    };
    let mut ui = Ui::new(store, view, FixedMetrics::default());
    let (x, y) = center(&ui, "a");
    click(&mut ui, x, y);
    assert_eq!(ui.focused(), Some("a"));
    assert!(
        !ui.draw()
            .iter()
            .any(|c| matches!(c, DrawCmd::Outline { .. })),
        "a click focuses without a ring"
    );
    ui.key(Key::Tab);
    assert!(
        ui.draw()
            .iter()
            .any(|c| matches!(c, DrawCmd::Outline { .. })),
        "Tab shows the ring"
    );
}
