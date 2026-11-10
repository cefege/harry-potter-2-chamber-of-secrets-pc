//! The runtime: rebuilds the view when state changed, lays it out, and turns pointer, keyboard and
//! text input into widget behaviour (hover, focus, press, drag, popups, text editing).
use crate::{
    layout::{TextMetrics, layout_root},
    node::{Kind, Node},
    paint::{DrawCmd, PaintState, Skin, format_number, paint, popup_rect, popup_row, texts},
    signal::Store,
    style::{Font, Rect, Style, Stylesheet},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerButton {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Tab,
    BackTab,
    Enter,
    Space,
    Escape,
    Left,
    Right,
    Up,
    Down,
    Backspace,
    Delete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tag {
    Button,
    Checkbox,
    Slider,
    Select,
    Number,
}

struct Edit {
    key: String,
    text: String,
    /// The first typed character replaces the whole text (like a selected field).
    fresh: bool,
}

struct Target {
    key: String,
    tag: Tag,
    rect: Rect,
}

/// A UI instance: one view function plus its interaction state.
pub struct Ui {
    store: Store,
    sheet: Stylesheet,
    skin: Skin,
    metrics: Box<dyn TextMetrics>,
    view: Box<dyn Fn() -> Node>,
    root: Node,
    built_at: Option<u64>,
    size: (f32, f32),
    hover: Option<String>,
    active: Option<String>,
    focus: Option<String>,
    open: Option<(String, usize)>,
    edit: Option<Edit>,
}

fn resolve(node: &mut Node, sheet: &Stylesheet) {
    let mut style = node
        .class
        .as_deref()
        .and_then(|c| sheet.get(c))
        .cloned()
        .unwrap_or_else(Style::default);
    for modify in &node.mods {
        modify(&mut style);
    }
    node.style = style;
    for child in &mut node.children {
        resolve(child, sheet);
    }
}

fn assign_keys(node: &mut Node, path: &str) {
    let key = node.key.clone().unwrap_or_else(|| path.to_owned());
    for (index, child) in node.children.iter_mut().enumerate() {
        assign_keys(child, &format!("{key}/{index}"));
    }
    node.key = Some(key);
}

fn collect(node: &Node, out: &mut Vec<Target>) {
    let tag = match node.kind {
        Kind::Button(_) => Some(Tag::Button),
        Kind::Checkbox { .. } => Some(Tag::Checkbox),
        Kind::Slider { .. } => Some(Tag::Slider),
        Kind::Select { .. } => Some(Tag::Select),
        Kind::NumberInput { .. } => Some(Tag::Number),
        Kind::Box | Kind::Text(_) => None,
    };
    if let (Some(tag), false, Some(key)) = (tag, node.disabled, &node.key) {
        out.push(Target {
            key: key.clone(),
            tag,
            rect: node.rect,
        });
    }
    for child in &node.children {
        collect(child, out);
    }
}

fn find<'a>(node: &'a Node, key: &str) -> Option<&'a Node> {
    if node.key.as_deref() == Some(key) {
        return Some(node);
    }
    node.children.iter().find_map(|c| find(c, key))
}

impl Ui {
    pub fn new(
        store: Store,
        view: impl Fn() -> Node + 'static,
        metrics: impl TextMetrics + 'static,
    ) -> Self {
        let mut ui = Self {
            store,
            sheet: Stylesheet::new(),
            skin: Skin::new(),
            metrics: Box::new(metrics),
            view: Box::new(view),
            root: crate::node::col(),
            built_at: None,
            size: (640., 480.),
            hover: None,
            active: None,
            focus: None,
            open: None,
            edit: None,
        };
        ui.rebuild();
        ui
    }

    pub fn with_stylesheet(mut self, sheet: Stylesheet) -> Self {
        self.sheet = sheet;
        self.rebuild();
        self
    }

    pub fn with_skin(mut self, skin: Skin) -> Self {
        self.skin = skin;
        self
    }

    /// Sets the viewport size in UI units and re-lays out.
    pub fn resize(&mut self, width: f32, height: f32) {
        if self.size != (width, height) {
            self.size = (width, height);
            self.rebuild();
        }
    }

    /// Rebuilds the tree if any signal changed since the last build. Returns whether it did.
    pub fn update(&mut self) -> bool {
        if self.built_at == Some(self.store.revision()) {
            return false;
        }
        self.rebuild();
        true
    }

    fn rebuild(&mut self) {
        let mut root = (self.view)();
        resolve(&mut root, &self.sheet);
        assign_keys(&mut root, "root");
        layout_root(&mut root, self.size.0, self.size.1, self.metrics.as_ref());
        self.root = root;
        self.built_at = Some(self.store.revision());
        // Interaction state must not outlive the nodes it refers to.
        let live = |key: &Option<String>, root: &Node| {
            key.as_deref().is_some_and(|k| find(root, k).is_some())
        };
        if !live(&self.hover, &self.root) {
            self.hover = None;
        }
        if !live(&self.active, &self.root) {
            self.active = None;
        }
        if !live(&self.focus, &self.root) {
            self.focus = None;
        }
        if self
            .open
            .as_ref()
            .is_some_and(|(k, _)| find(&self.root, k).is_none())
        {
            self.open = None;
        }
        if self
            .edit
            .as_ref()
            .is_some_and(|e| find(&self.root, &e.key).is_none())
        {
            self.edit = None;
        }
    }

    pub fn root(&self) -> &Node {
        &self.root
    }

    /// The laid-out rectangle of the node with this key.
    pub fn rect_of(&self, key: &str) -> Option<Rect> {
        find(&self.root, key).map(|n| n.rect)
    }

    pub fn focused(&self) -> Option<&str> {
        self.focus.as_deref()
    }

    pub fn open_select(&self) -> Option<&str> {
        self.open.as_ref().map(|(k, _)| k.as_str())
    }

    /// Every string (with its font) the current tree draws, so a host can measure them first.
    pub fn text_requests(&self) -> Vec<(String, Font)> {
        let mut out = Vec::new();
        texts(&self.root, &mut out);
        out
    }

    /// Lays the current tree out again (after the host learned new text measurements).
    pub fn relayout(&mut self) {
        layout_root(
            &mut self.root,
            self.size.0,
            self.size.1,
            self.metrics.as_ref(),
        );
    }

    /// Whether a number box is being typed in (the host should then route text input here).
    pub fn is_editing(&self) -> bool {
        self.edit.is_some()
    }

    /// The draw list for the current state.
    pub fn draw(&mut self) -> Vec<DrawCmd> {
        self.update();
        let state = PaintState {
            hover: self.hover.as_deref(),
            active: self.active.as_deref(),
            focus: self.focus.as_deref(),
            open: self.open.as_ref().map(|(k, i)| (k.as_str(), *i)),
            edit: self
                .edit
                .as_ref()
                .map(|e| (e.key.as_str(), e.text.as_str())),
        };
        paint(&self.root, &state, &self.skin, self.metrics.as_ref())
    }

    fn targets(&self) -> Vec<Target> {
        let mut out = Vec::new();
        collect(&self.root, &mut out);
        out
    }

    fn hit(&self, x: f32, y: f32) -> Option<Target> {
        self.targets()
            .into_iter()
            .rev()
            .find(|t| t.rect.contains(x, y))
    }

    // ---- pointer ---------------------------------------------------------------------------

    pub fn pointer_move(&mut self, x: f32, y: f32) {
        self.hover = self.hit(x, y).map(|t| t.key);
        if let Some((key, _)) = self.open.clone()
            && let Some((anchor, count)) = find(&self.root, &key).and_then(|n| {
                if let Kind::Select { options, .. } = &n.kind {
                    Some((n.rect, options.len()))
                } else {
                    None
                }
            })
            && let Some(index) =
                (0..count).find(|&i| popup_row(anchor, i, &self.skin).contains(x, y))
        {
            self.open = Some((key, index));
        }
        if let Some(key) = self.active.clone() {
            self.drag_slider(&key, x);
        }
        self.update();
    }

    pub fn pointer_down(&mut self, x: f32, y: f32, button: PointerButton) {
        if button != PointerButton::Left {
            return;
        }
        // An open drop-down owns the next click: it picks a row or closes.
        if let Some((key, _)) = self.open.clone() {
            let anchor = find(&self.root, &key).and_then(|n| {
                if let Kind::Select {
                    options,
                    selected,
                    on_select,
                } = &n.kind
                {
                    Some((n.rect, options.len(), *selected, on_select.clone()))
                } else {
                    None
                }
            });
            if let Some((rect, count, _, on_select)) = anchor {
                self.open = None;
                if popup_rect(rect, count, &self.skin).contains(x, y) {
                    if let Some(index) =
                        (0..count).find(|&i| popup_row(rect, i, &self.skin).contains(x, y))
                    {
                        on_select(index);
                    }
                    self.update();
                    return;
                }
                if rect.contains(x, y) {
                    self.update();
                    return;
                }
            }
        }
        let target = self.hit(x, y);
        let key = target.as_ref().map(|t| t.key.clone());
        if self.focus != key {
            self.commit_edit();
        }
        self.focus = key.clone();
        self.active = key.clone();
        if let Some(target) = target {
            match target.tag {
                Tag::Slider => self.drag_slider(&target.key, x),
                Tag::Number => self.begin_edit(&target.key),
                // Buttons, checkboxes and drop-downs act on release (see `pointer_up`).
                Tag::Select | Tag::Button | Tag::Checkbox => {}
            }
        }
        self.update();
    }

    pub fn pointer_up(&mut self, x: f32, y: f32, button: PointerButton) {
        if button != PointerButton::Left {
            return;
        }
        let pressed = self.active.take();
        let released_on = self.hit(x, y).map(|t| t.key);
        if let (Some(pressed), Some(released)) = (pressed, released_on)
            && pressed == released
        {
            self.activate(&pressed);
        }
        self.update();
    }

    fn drag_slider(&mut self, key: &str, x: f32) {
        let Some(node) = find(&self.root, key) else {
            return;
        };
        let Kind::Slider {
            min,
            max,
            step,
            value,
            on_change,
        } = &node.kind
        else {
            return;
        };
        let rect = node.rect;
        let m = self.skin.slider;
        let frac =
            ((x - rect.x - m.thumb_start - m.thumb_width / 2.) / m.travel.max(1.)).clamp(0., 1.);
        let mut next = min + frac * (max - min);
        if *step > 0. {
            next = (min + ((next - min) / step).round() * step).clamp(*min, *max);
        }
        if (next - *value).abs() > f32::EPSILON {
            let on_change = on_change.clone();
            on_change(next);
        }
    }

    /// A click or Enter/Space on a widget.
    fn activate(&mut self, key: &str) {
        let Some(node) = find(&self.root, key) else {
            return;
        };
        match &node.kind {
            Kind::Button(_) => {
                if let Some(handler) = node.on_click.clone() {
                    handler();
                }
            }
            Kind::Checkbox {
                checked, on_toggle, ..
            } => {
                let (on_toggle, next) = (on_toggle.clone(), !*checked);
                on_toggle(next);
            }
            Kind::Select { selected, .. } => {
                self.open = if self.open.as_ref().is_some_and(|(k, _)| k == key) {
                    None
                } else {
                    Some((key.to_owned(), *selected))
                };
            }
            Kind::Slider { .. } | Kind::NumberInput { .. } | Kind::Box | Kind::Text(_) => {}
        }
    }

    // ---- keyboard --------------------------------------------------------------------------

    pub fn key(&mut self, key: Key) {
        match key {
            Key::Tab | Key::BackTab => self.move_focus(key == Key::Tab),
            Key::Escape => {
                if self.open.take().is_none() {
                    self.edit = None;
                }
            }
            _ => self.key_on_focused(key),
        }
        self.update();
    }

    fn move_focus(&mut self, forward: bool) {
        self.commit_edit();
        self.open = None;
        let keys: Vec<String> = self.targets().into_iter().map(|t| t.key).collect();
        if keys.is_empty() {
            return;
        }
        let current = self
            .focus
            .as_ref()
            .and_then(|f| keys.iter().position(|k| k == f));
        let next = match (current, forward) {
            (None, true) => 0,
            (None, false) => keys.len() - 1,
            (Some(i), true) => (i + 1) % keys.len(),
            (Some(i), false) => (i + keys.len() - 1) % keys.len(),
        };
        self.focus = Some(keys[next].clone());
        if find(&self.root, &keys[next]).is_some_and(|n| matches!(n.kind, Kind::NumberInput { .. }))
        {
            self.begin_edit(&keys[next].clone());
        }
    }

    fn key_on_focused(&mut self, key: Key) {
        let Some(focus) = self.focus.clone() else {
            return;
        };
        if self.edit.as_ref().is_some_and(|e| e.key == focus) {
            match key {
                Key::Backspace | Key::Delete => self.edit_backspace(),
                Key::Enter => self.commit_edit(),
                _ => {}
            }
            return;
        }
        let Some(node) = find(&self.root, &focus) else {
            return;
        };
        match (&node.kind, key) {
            (Kind::Button(_) | Kind::Checkbox { .. }, Key::Enter | Key::Space) => {
                self.activate(&focus)
            }
            (
                Kind::Slider {
                    min,
                    max,
                    step,
                    value,
                    on_change,
                },
                Key::Left | Key::Right | Key::Down | Key::Up,
            ) => {
                let delta = if *step > 0. { *step } else { (max - min) / 20. };
                let sign = if matches!(key, Key::Left | Key::Down) {
                    -1.
                } else {
                    1.
                };
                let next = (value + sign * delta).clamp(*min, *max);
                if (next - value).abs() > f32::EPSILON {
                    let on_change = on_change.clone();
                    on_change(next);
                }
            }
            (
                Kind::Select {
                    options,
                    selected,
                    on_select,
                },
                Key::Up | Key::Down | Key::Left | Key::Right,
            ) => {
                let count = options.len();
                if count == 0 {
                    return;
                }
                let backwards = matches!(key, Key::Up | Key::Left);
                if let Some((_, highlight)) = self.open.clone() {
                    let next = if backwards {
                        highlight.saturating_sub(1)
                    } else {
                        (highlight + 1).min(count - 1)
                    };
                    self.open = Some((focus, next));
                } else {
                    let next = if backwards {
                        (selected + count - 1) % count
                    } else {
                        (selected + 1) % count
                    };
                    let on_select = on_select.clone();
                    on_select(next);
                }
            }
            (Kind::Select { on_select, .. }, Key::Enter | Key::Space) => {
                if let Some((_, highlight)) = self.open.take() {
                    let on_select = on_select.clone();
                    on_select(highlight);
                } else {
                    self.activate(&focus);
                }
            }
            (Kind::NumberInput { .. }, Key::Enter | Key::Space) => self.begin_edit(&focus),
            _ => {}
        }
    }

    // ---- text editing ------------------------------------------------------------------------

    /// Typed text (from the platform's text-input events).
    pub fn text(&mut self, typed: &str) {
        let Some(edit_key) = self.edit.as_ref().map(|e| e.key.clone()) else {
            return;
        };
        let Some((min, decimals)) = find(&self.root, &edit_key).and_then(|n| {
            if let Kind::NumberInput { min, decimals, .. } = &n.kind {
                Some((*min, *decimals))
            } else {
                None
            }
        }) else {
            return;
        };
        for ch in typed.chars() {
            let Some(edit) = self.edit.as_mut() else {
                return;
            };
            let fraction_digits = edit.text.split_once('.').map_or(0, |(_, f)| f.len());
            let accepted = match ch {
                '0'..='9' => !edit.text.contains('.') || edit.fresh || fraction_digits < decimals,
                '.' | ',' => decimals > 0 && !edit.text.contains('.') && !edit.fresh,
                '-' => min < 0. && (edit.text.is_empty() || edit.fresh),
                _ => false,
            };
            // A decimal point right after select-all starts "0." rather than being dropped.
            let starts_fraction = matches!(ch, '.' | ',') && decimals > 0 && edit.fresh;
            if !accepted && !starts_fraction {
                continue;
            }
            if edit.fresh {
                edit.text.clear();
                edit.fresh = false;
            }
            if starts_fraction {
                edit.text.push('0');
            }
            if edit.text.len() < 8 {
                edit.text.push(if ch == ',' { '.' } else { ch });
            }
        }
        self.apply_edit(false);
        self.update();
    }

    fn begin_edit(&mut self, key: &str) {
        if self.edit.as_ref().is_some_and(|e| e.key == key) {
            return;
        }
        let Some(text) = find(&self.root, key).and_then(|n| {
            if let Kind::NumberInput {
                value, decimals, ..
            } = &n.kind
            {
                Some(format_number(*value, *decimals))
            } else {
                None
            }
        }) else {
            return;
        };
        self.edit = Some(Edit {
            key: key.to_owned(),
            text,
            fresh: true,
        });
    }

    fn edit_backspace(&mut self) {
        if let Some(edit) = self.edit.as_mut() {
            if edit.fresh {
                edit.text.clear();
                edit.fresh = false;
            } else {
                edit.text.pop();
            }
        }
        self.apply_edit(false);
    }

    /// Applies the buffer if it is a number in range. With `clamp`, an out-of-range number is
    /// pulled into range first (used when the edit ends).
    fn apply_edit(&mut self, clamp: bool) {
        let Some(edit) = &self.edit else { return };
        let Some(node) = find(&self.root, &edit.key) else {
            return;
        };
        let Kind::NumberInput {
            min,
            max,
            on_change,
            ..
        } = &node.kind
        else {
            return;
        };
        let Ok(mut value) = edit.text.parse::<f32>() else {
            return;
        };
        if !value.is_finite() {
            return;
        }
        if clamp {
            value = value.clamp(*min, *max);
        }
        if (*min..=*max).contains(&value) {
            let on_change = on_change.clone();
            on_change(value);
        }
    }

    /// Ends editing, keeping a usable number (clamped) and reverting unusable text.
    fn commit_edit(&mut self) {
        if self.edit.is_some() {
            self.apply_edit(true);
            self.edit = None;
        }
    }
}
