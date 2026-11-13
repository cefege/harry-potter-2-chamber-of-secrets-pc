//! Geometry, colour and the style vocabulary (a small CSS subset).
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
}

/// Straight-alpha RGBA, 0..=255.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

impl Color {
    pub const WHITE: Color = Color(255, 255, 255, 255);
    pub const TRANSPARENT: Color = Color(0, 0, 0, 0);
}

impl Default for Color {
    fn default() -> Self {
        Color::WHITE
    }
}

/// Image compositing policy; `Inherit` leaves the host's current mode unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BlendMode {
    #[default]
    Inherit,
    Invisible,
    Opaque,
    Masked,
    Translucent,
    Modulated,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    Row,
    #[default]
    Column,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
}

/// Width or height of a box.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Size {
    /// Fit the content (the widget's natural size).
    #[default]
    Auto,
    Px(f32),
    /// Share of the free space along the main axis, like `flex-grow`.
    Grow(f32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    pub fn all(v: f32) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }

    pub fn xy(x: f32, y: f32) -> Self {
        Self {
            top: y,
            right: x,
            bottom: y,
            left: x,
        }
    }

    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Font role; the host maps it to a real face and size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Font {
    #[default]
    Normal,
    Bold,
    Heading,
    /// A host font slot (for the game: `Root.Fonts[n]`).
    Slot(u8),
}

/// Box style. Unset fields keep their defaults; a [`Stylesheet`] class is applied first and the
/// node's own style overrides it field by field via the builder methods.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Style {
    pub direction: Direction,
    pub gap: f32,
    pub padding: Edges,
    pub width: Size,
    pub height: Size,
    pub align: Align,
    pub justify: Justify,
    pub color: Color,
    pub blend: BlendMode,
    pub background: Option<Color>,
    pub font: Font,
    pub text_align: TextAlign,
    /// Lay the node out at `(x, y)` inside its parent instead of in the flow (popups, overlays).
    pub absolute: Option<(f32, f32)>,
}

/// Named styles, the analogue of a CSS file: `.class("heading")` looks the name up.
#[derive(Clone, Debug, Default)]
pub struct Stylesheet {
    classes: HashMap<String, Style>,
}

impl Stylesheet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, class: &str, style: Style) -> Self {
        self.classes.insert(class.to_owned(), style);
        self
    }

    pub fn get(&self, class: &str) -> Option<&Style> {
        self.classes.get(class)
    }
}
