//! The view tree and its builders. A view function returns a [`Node`]; nothing here touches input
//! or painting.
use std::rc::Rc;

use crate::style::{Align, BlendMode, Color, Direction, Edges, Font, Justify, Rect, Size, Style, TextAlign};

pub(crate) type Click = Rc<dyn Fn()>;
pub(crate) type Toggle = Rc<dyn Fn(bool)>;
pub(crate) type Change = Rc<dyn Fn(f32)>;
pub(crate) type Pick = Rc<dyn Fn(usize)>;
pub(crate) type Hover = Rc<dyn Fn(bool)>;
type StyleMod = Rc<dyn Fn(&mut Style)>;

pub(crate) enum Kind {
    Box,
    Text(String),
    Image(crate::paint::TextureRef, crate::paint::Fit),
    Button(String),
    Checkbox {
        text: String,
        checked: bool,
        on_toggle: Toggle,
    },
    Slider {
        min: f32,
        max: f32,
        step: f32,
        value: f32,
        on_change: Change,
    },
    Select {
        options: Vec<String>,
        selected: usize,
        on_select: Pick,
    },
    NumberInput {
        value: f32,
        min: f32,
        max: f32,
        decimals: usize,
        on_change: Change,
    },
    ImageButton(Box<ImageButton>),
}

/// One element of the UI tree.
pub struct Node {
    pub(crate) kind: Kind,
    pub(crate) class: Option<String>,
    pub(crate) mods: Vec<StyleMod>,
    /// The resolved style (class, then modifiers); filled in by the `Ui` before layout.
    pub(crate) style: Style,
    pub(crate) key: Option<String>,
    pub(crate) children: Vec<Node>,
    pub(crate) on_click: Option<Click>,
    pub(crate) on_hover: Option<Hover>,
    pub(crate) disabled: bool,
    pub(crate) rect: Rect,
}

impl Node {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            class: None,
            mods: Vec::new(),
            style: Style::default(),
            key: None,
            children: Vec::new(),
            on_click: None,
            on_hover: None,
            disabled: false,
            rect: Rect::default(),
        }
    }

    fn modify(mut self, change: impl Fn(&mut Style) + 'static) -> Self {
        self.mods.push(Rc::new(change));
        self
    }

    /// Applies the named class from the `Stylesheet`; later builder calls override it.
    pub fn class(mut self, name: &str) -> Self {
        self.class = Some(name.to_owned());
        self
    }

    /// A stable identity for focus and editing state across rebuilds. Give one to every
    /// interactive widget whose position in the tree can change.
    pub fn key(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }

    pub fn style(self, style: Style) -> Self {
        self.modify(move |s| *s = style.clone())
    }

    pub fn width(self, size: Size) -> Self {
        self.modify(move |s| s.width = size)
    }

    pub fn height(self, size: Size) -> Self {
        self.modify(move |s| s.height = size)
    }

    pub fn gap(self, gap: f32) -> Self {
        self.modify(move |s| s.gap = gap)
    }

    pub fn padding(self, padding: Edges) -> Self {
        self.modify(move |s| s.padding = padding)
    }

    pub fn align(self, align: Align) -> Self {
        self.modify(move |s| s.align = align)
    }

    pub fn justify(self, justify: Justify) -> Self {
        self.modify(move |s| s.justify = justify)
    }

    pub fn color(self, color: Color) -> Self {
        self.modify(move |s| s.color = color)
    }

    /// Sets image compositing without changing text paint policy.
    pub fn blend(self, blend: BlendMode) -> Self {
        self.modify(move |s| s.blend = blend)
    }

    pub fn background(self, color: Color) -> Self {
        self.modify(move |s| s.background = Some(color))
    }

    pub fn font(self, font: Font) -> Self {
        self.modify(move |s| s.font = font)
    }

    pub fn text_align(self, align: TextAlign) -> Self {
        self.modify(move |s| s.text_align = align)
    }

    /// Position inside the parent, outside the flow.
    pub fn at(self, x: f32, y: f32) -> Self {
        self.modify(move |s| s.absolute = Some((x, y)))
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn child(mut self, child: Node) -> Self {
        self.children.push(child);
        self
    }

    pub fn children(mut self, children: impl IntoIterator<Item = Node>) -> Self {
        self.children.extend(children);
        self
    }

    /// Click handler for a `button` (and any future clickable box).
    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Called with `true` when the pointer enters this node and `false` when it leaves.
    pub fn on_hover(mut self, handler: impl Fn(bool) + 'static) -> Self {
        self.on_hover = Some(Rc::new(handler));
        self
    }

    /// Where layout placed this node (valid after the `Ui` laid out the tree).
    pub fn rect(&self) -> Rect {
        self.rect
    }
}

/// A vertical flex container.
pub fn col() -> Node {
    Node::new(Kind::Box).modify(|s| s.direction = Direction::Column)
}

/// A horizontal flex container.
pub fn row() -> Node {
    Node::new(Kind::Box).modify(|s| s.direction = Direction::Row)
}

/// Empty space that grows to fill the free space of its parent.
pub fn spacer() -> Node {
    Node::new(Kind::Box)
        .width(Size::Grow(1.))
        .height(Size::Grow(1.))
}

pub fn label(text: &str) -> Node {
    Node::new(Kind::Text(text.to_owned()))
}

/// Non-interactive artwork; the host supplies the texture and the view supplies its size.
pub fn image(texture: crate::paint::TextureRef, fit: crate::paint::Fit) -> Node {
    Node::new(Kind::Image(texture, fit))
}

pub fn button(text: &str) -> Node {
    Node::new(Kind::Button(text.to_owned()))
}

pub fn checkbox(text: &str, checked: bool, on_toggle: impl Fn(bool) + 'static) -> Node {
    Node::new(Kind::Checkbox {
        text: text.to_owned(),
        checked,
        on_toggle: Rc::new(on_toggle),
    })
}

/// A horizontal slider over `min..=max`, snapped to `step` (0 for continuous).
pub fn slider(
    min: f32,
    max: f32,
    step: f32,
    value: f32,
    on_change: impl Fn(f32) + 'static,
) -> Node {
    Node::new(Kind::Slider {
        min,
        max,
        step,
        value,
        on_change: Rc::new(on_change),
    })
}

/// A drop-down list.
pub fn select<S: AsRef<str>>(
    options: &[S],
    selected: usize,
    on_select: impl Fn(usize) + 'static,
) -> Node {
    let options = options.iter().map(|s| s.as_ref().to_owned()).collect();
    Node::new(Kind::Select {
        options,
        selected,
        on_select: Rc::new(on_select),
    })
}

/// An editable number box. `on_change` fires on every keystroke whose text is a number in
/// `min..=max`, so dependent state updates live; text outside the range is kept but not applied.
pub fn number_input(
    value: f32,
    min: f32,
    max: f32,
    decimals: usize,
    on_change: impl Fn(f32) + 'static,
) -> Node {
    Node::new(Kind::NumberInput {
        value,
        min,
        max,
        decimals,
        on_change: Rc::new(on_change),
    })
}

impl Kind {
    pub(crate) fn focusable(&self) -> bool {
        !matches!(self, Kind::Box | Kind::Text(_) | Kind::Image(..))
    }
}

/// Art for one button state: a texture and how it is fitted.
#[derive(Clone, Debug, PartialEq)]
pub struct ButtonArt {
    pub texture: crate::paint::TextureRef,
    pub fit: crate::paint::Fit,
    /// Offset of the art inside the button.
    pub offset: (f32, f32),
    /// Drawn size for `Fit::Region` and `Fit::Stretch` (defaults to the button size).
    pub size: Option<(f32, f32)>,
}

/// A button drawn with per-state art and an optional caption (the stock `UWindowButton`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImageButton {
    pub up: Option<ButtonArt>,
    pub over: Option<ButtonArt>,
    pub down: Option<ButtonArt>,
    pub disabled: Option<ButtonArt>,
    pub text: String,
    pub over_color: Option<Color>,
    pub shadow: bool,
    pub text_align: TextAlign,
    /// Horizontal inset of left- or right-aligned text.
    pub text_inset: f32,
    /// Exact top-left of the caption inside the button; overrides alignment.
    pub text_pos: Option<(f32, f32)>,
}

/// A button drawn with art; give it a fixed width and height.
pub fn image_button(button: ImageButton) -> Node {
    Node::new(Kind::ImageButton(Box::new(button)))
}
