//! A small retained-mode, reactive UI toolkit for the game's menus.
//!
//! It plays the role HTML plus a framework plays on the web, without a browser:
//!
//! * **State** lives in [`Signal`]s created from a [`Store`]. Setting a signal to a different value
//!   bumps the store's revision.
//! * **Views** are plain functions that return a [`Node`] tree (`col`, `row`, `label`, `button`,
//!   `checkbox`, `slider`, `select`, `number_input`) styled with a [`Style`] and a [`Stylesheet`] of
//!   classes. Event handlers are closures that set signals.
//! * A [`Ui`] rebuilds the tree from the view when the revision changed, lays it out with a
//!   flexbox-style engine, routes pointer, keyboard and text input (hover, focus, drag, popups, text
//!   editing) and paints to a backend-neutral [`DrawCmd`] list.
//!
//! The crate has no engine, GPU or font dependency. The host supplies [`TextMetrics`] and turns
//! [`DrawCmd`]s into pixels, so the toolkit is unit-testable on its own.

mod layout;
mod node;
mod paint;
mod signal;
mod style;
mod ui;

pub use layout::{FixedMetrics, TextMetrics};
pub use node::{
    ButtonArt, ImageButton, Node, button, checkbox, col, image_button, label, number_input, row,
    select, slider, spacer,
};
pub use paint::{DrawCmd, Fit, PopupMetrics, Skin, SkinPart, SliderMetrics, TextureRef};
pub use signal::{Signal, Store};
pub use style::{
    Align, Color, Direction, Edges, Font, Justify, Rect, Size, Style, Stylesheet, TextAlign,
};
pub use ui::{Key, PointerButton, Ui};
