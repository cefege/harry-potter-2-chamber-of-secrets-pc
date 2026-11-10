//! Backend-neutral drawing: the tree becomes a flat list of [`DrawCmd`]s.
use std::collections::HashMap;

use crate::{
    layout::{CHECK_GAP, CHECK_SIZE, TextMetrics, WIDGET_HEIGHT},
    node::{Kind, Node},
    style::{Color, Font, Rect},
};

/// Names a texture in the host (for the game: `"HGame.FEComboIdleTexture"`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TextureRef(pub String);

/// The art-bearing pieces of the built-in widgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SkinPart {
    ButtonIdle,
    ButtonHover,
    ButtonDown,
    BoxIdle,
    BoxHover,
    ListBackground,
    ListRowSelected,
    SliderTrack,
    SliderThumb,
    CheckOff,
    CheckOffHover,
    CheckOn,
    CheckOnHover,
}

/// Which texture draws each [`SkinPart`]. A missing part paints a flat fallback rectangle.
#[derive(Clone, Debug, Default)]
pub struct Skin {
    parts: HashMap<SkinPart, TextureRef>,
}

impl Skin {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, part: SkinPart, texture: &str) -> Self {
        self.parts.insert(part, TextureRef(texture.to_owned()));
        self
    }

    pub fn get(&self, part: SkinPart) -> Option<&TextureRef> {
        self.parts.get(&part)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
}

/// One drawing operation, in paint order.
#[derive(Clone, Debug, PartialEq)]
pub enum DrawCmd {
    Rect {
        rect: Rect,
        color: Color,
    },
    Outline {
        rect: Rect,
        color: Color,
    },
    /// A skin texture stretched over `rect`.
    Image {
        texture: TextureRef,
        rect: Rect,
    },
    Text {
        rect: Rect,
        text: String,
        font: Font,
        color: Color,
        align: TextAlign,
    },
    PushClip(Rect),
    PopClip,
}

/// Interaction state the painter needs (borrowed from the `Ui`).
#[derive(Default)]
pub(crate) struct PaintState<'a> {
    pub hover: Option<&'a str>,
    pub active: Option<&'a str>,
    pub focus: Option<&'a str>,
    /// Open drop-down: its key and the highlighted option.
    pub open: Option<(&'a str, usize)>,
    /// Text being edited: key and buffer.
    pub edit: Option<(&'a str, &'a str)>,
}

pub(crate) const ROW_HEIGHT: f32 = WIDGET_HEIGHT;
const FOCUS: Color = Color(255, 214, 90, 255);
const FALLBACK_BOX: Color = Color(120, 70, 20, 255);
const FALLBACK_HOVER: Color = Color(220, 120, 30, 255);
const FALLBACK_TRACK: Color = Color(30, 140, 30, 255);
const FALLBACK_THUMB: Color = Color(200, 140, 40, 255);
const FALLBACK_LIST: Color = Color(70, 40, 12, 255);
const FALLBACK_ROW: Color = Color(220, 120, 30, 255);
const VALUE_TEXT: Color = Color(220, 220, 220, 255);

fn part(out: &mut Vec<DrawCmd>, skin: &Skin, part: SkinPart, rect: Rect, fallback: Color) {
    match skin.get(part) {
        Some(texture) => out.push(DrawCmd::Image {
            texture: texture.clone(),
            rect,
        }),
        None => out.push(DrawCmd::Rect {
            rect,
            color: fallback,
        }),
    }
}

fn text(
    out: &mut Vec<DrawCmd>,
    rect: Rect,
    text: &str,
    font: Font,
    color: Color,
    align: TextAlign,
) {
    out.push(DrawCmd::Text {
        rect,
        text: text.to_owned(),
        font,
        color,
        align,
    });
}

/// The popup rectangle of an open drop-down.
pub(crate) fn popup_rect(anchor: Rect, options: usize) -> Rect {
    Rect::new(
        anchor.x,
        anchor.bottom(),
        anchor.w,
        ROW_HEIGHT * options as f32 + 6.,
    )
}

/// Where option `index` of a popup is.
pub(crate) fn popup_row(anchor: Rect, index: usize) -> Rect {
    Rect::new(
        anchor.x + 2.,
        anchor.bottom() + 3. + ROW_HEIGHT * index as f32,
        anchor.w - 4.,
        ROW_HEIGHT,
    )
}

pub(crate) fn slider_thumb(rect: Rect, min: f32, max: f32, value: f32) -> Rect {
    let width = 10.;
    let frac = if max > min {
        ((value - min) / (max - min)).clamp(0., 1.)
    } else {
        0.
    };
    Rect::new(rect.x + frac * (rect.w - width), rect.y, width, rect.h)
}

pub(crate) fn format_number(value: f32, decimals: usize) -> String {
    format!("{value:.decimals$}")
}

/// Paints the tree. Popups of open drop-downs are painted last, on top.
pub(crate) fn paint(
    root: &Node,
    state: &PaintState<'_>,
    skin: &Skin,
    metrics: &dyn TextMetrics,
) -> Vec<DrawCmd> {
    let mut out = Vec::new();
    let mut popups = Vec::new();
    paint_node(root, state, skin, metrics, &mut out, &mut popups);
    out.extend(popups);
    out
}

fn paint_node(
    node: &Node,
    state: &PaintState<'_>,
    skin: &Skin,
    metrics: &dyn TextMetrics,
    out: &mut Vec<DrawCmd>,
    popups: &mut Vec<DrawCmd>,
) {
    let key = node.key.as_deref().unwrap_or_default();
    let rect = node.rect;
    let hover = state.hover == Some(key) && !node.disabled;
    let down = state.active == Some(key) && !node.disabled;
    let focused = state.focus == Some(key);
    let style = &node.style;
    match &node.kind {
        Kind::Box => {
            if let Some(color) = style.background {
                out.push(DrawCmd::Rect { rect, color });
            }
        }
        Kind::Text(content) => text(out, rect, content, style.font, style.color, TextAlign::Left),
        Kind::Button(content) => {
            let which = if down {
                SkinPart::ButtonDown
            } else if hover {
                SkinPart::ButtonHover
            } else {
                SkinPart::ButtonIdle
            };
            part(
                out,
                skin,
                which,
                rect,
                if hover { FALLBACK_HOVER } else { FALLBACK_BOX },
            );
            text(
                out,
                rect,
                content,
                style.font,
                style.color,
                TextAlign::Center,
            );
        }
        Kind::Checkbox {
            text: content,
            checked,
            ..
        } => {
            let size = CHECK_SIZE;
            let box_rect = Rect::new(rect.x, rect.y + (rect.h - size) / 2., size, size);
            let which = match (*checked, hover) {
                (false, false) => SkinPart::CheckOff,
                (false, true) => SkinPart::CheckOffHover,
                (true, false) => SkinPart::CheckOn,
                (true, true) => SkinPart::CheckOnHover,
            };
            part(
                out,
                skin,
                which,
                box_rect,
                if *checked {
                    FALLBACK_HOVER
                } else {
                    FALLBACK_BOX
                },
            );
            let label_rect = Rect::new(
                rect.x + size + CHECK_GAP,
                rect.y,
                (rect.w - size - CHECK_GAP).max(0.),
                rect.h,
            );
            text(
                out,
                label_rect,
                content,
                style.font,
                style.color,
                TextAlign::Left,
            );
        }
        Kind::Slider {
            min, max, value, ..
        } => {
            part(out, skin, SkinPart::SliderTrack, rect, FALLBACK_TRACK);
            part(
                out,
                skin,
                SkinPart::SliderThumb,
                slider_thumb(rect, *min, *max, *value),
                FALLBACK_THUMB,
            );
        }
        Kind::Select {
            options, selected, ..
        } => {
            part(
                out,
                skin,
                if hover {
                    SkinPart::BoxHover
                } else {
                    SkinPart::BoxIdle
                },
                rect,
                if hover { FALLBACK_HOVER } else { FALLBACK_BOX },
            );
            let value_rect = Rect::new(rect.x + 8., rect.y, rect.w - 16., rect.h);
            text(
                out,
                value_rect,
                options.get(*selected).map_or("", String::as_str),
                style.font,
                VALUE_TEXT,
                TextAlign::Left,
            );
            if let Some((open_key, highlight)) = state.open.filter(|(k, _)| *k == key) {
                let _ = open_key;
                part(
                    popups,
                    skin,
                    SkinPart::ListBackground,
                    popup_rect(rect, options.len()),
                    FALLBACK_LIST,
                );
                for (index, option) in options.iter().enumerate() {
                    let row = popup_row(rect, index);
                    if index == highlight {
                        part(popups, skin, SkinPart::ListRowSelected, row, FALLBACK_ROW);
                    }
                    text(
                        popups,
                        Rect::new(row.x + 6., row.y, row.w - 12., row.h),
                        option,
                        style.font,
                        if index == highlight {
                            Color(0, 0, 0, 255)
                        } else {
                            Color::WHITE
                        },
                        TextAlign::Left,
                    );
                }
            }
        }
        Kind::NumberInput {
            value, decimals, ..
        } => {
            part(
                out,
                skin,
                if hover || focused {
                    SkinPart::BoxHover
                } else {
                    SkinPart::BoxIdle
                },
                rect,
                if hover || focused {
                    FALLBACK_HOVER
                } else {
                    FALLBACK_BOX
                },
            );
            let editing = state.edit.filter(|(k, _)| *k == key).map(|(_, t)| t);
            let shown = editing.map_or_else(|| format_number(*value, *decimals), str::to_owned);
            let value_rect = Rect::new(rect.x + 8., rect.y, rect.w - 16., rect.h);
            text(
                out,
                value_rect,
                &shown,
                style.font,
                VALUE_TEXT,
                TextAlign::Left,
            );
            if editing.is_some() {
                let (tw, th) = metrics.text_size(&shown, style.font);
                out.push(DrawCmd::Rect {
                    rect: Rect::new(value_rect.x + tw + 1., rect.y + (rect.h - th) / 2., 1., th),
                    color: VALUE_TEXT,
                });
            }
        }
    }
    if focused && node.kind.focusable() && !node.disabled {
        out.push(DrawCmd::Outline { rect, color: FOCUS });
    }
    for child in &node.children {
        paint_node(child, state, skin, metrics, out, popups);
    }
}
