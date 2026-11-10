//! Backend-neutral drawing: the tree becomes a flat list of [`DrawCmd`]s.
use std::collections::HashMap;

use crate::{
    layout::{CHECK_GAP, TextMetrics},
    node::{Kind, Node},
    style::{Color, Font, Rect, TextAlign},
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
    /// Drawn over the track while the pointer is over the slider (optional).
    SliderTrackHover,
    SliderThumb,
    CheckOff,
    CheckOffHover,
    CheckOn,
    CheckOnHover,
}

/// How a skin texture is drawn into its rectangle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Fit {
    /// Stretched over the whole rectangle.
    #[default]
    Stretch,
    /// At the texture's own size, anchored at the rectangle's top-left (the stock menus' padded
    /// power-of-two art relies on this).
    Native,
    /// A region of the texture (`u, v, width, height` in texels) stretched over the rectangle.
    Region(f32, f32, f32, f32),
}

/// Slider art placement relative to the slider's rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderMetrics {
    /// Horizontal offset of the track bitmap.
    pub track_dx: f32,
    /// Offset of the knob at the minimum value.
    pub thumb_start: f32,
    /// How far the knob travels from minimum to maximum.
    pub travel: f32,
    /// Width of the knob's visible art, for pointer-to-value mapping.
    pub thumb_width: f32,
}

impl Default for SliderMetrics {
    fn default() -> Self {
        Self {
            track_dx: 0.,
            thumb_start: 0.,
            travel: 123.,
            thumb_width: 10.,
        }
    }
}

/// Drop-down popup geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopupMetrics {
    pub row_height: f32,
    pub top: f32,
    pub bottom: f32,
    /// Horizontal inset of the highlighted row.
    pub inset: f32,
    /// Where option text starts inside a row.
    pub text_dx: f32,
}

impl Default for PopupMetrics {
    fn default() -> Self {
        Self {
            row_height: 24.,
            top: 3.,
            bottom: 3.,
            inset: 2.,
            text_dx: 8.,
        }
    }
}

/// Which texture draws each [`SkinPart`], plus the art's measurements. A missing part paints a
/// flat fallback rectangle.
#[derive(Clone, Debug)]
pub struct Skin {
    parts: HashMap<SkinPart, (TextureRef, Fit)>,
    pub slider: SliderMetrics,
    pub popup: PopupMetrics,
    /// Side of the checkbox art.
    pub check_size: f32,
    /// Vertical offset of the combo/box art.
    pub box_dy: f32,
    /// Offset of value text inside a box.
    pub box_text_dx: f32,
}

impl Default for Skin {
    fn default() -> Self {
        Self {
            parts: HashMap::new(),
            slider: SliderMetrics::default(),
            popup: PopupMetrics::default(),
            check_size: 16.,
            box_dy: 0.,
            box_text_dx: 8.,
        }
    }
}

impl Skin {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, part: SkinPart, texture: &str) -> Self {
        self.parts
            .insert(part, (TextureRef(texture.to_owned()), Fit::Stretch));
        self
    }

    pub fn with_native(mut self, part: SkinPart, texture: &str) -> Self {
        self.parts
            .insert(part, (TextureRef(texture.to_owned()), Fit::Native));
        self
    }

    pub fn with_slider(mut self, slider: SliderMetrics) -> Self {
        self.slider = slider;
        self
    }

    pub fn with_popup(mut self, popup: PopupMetrics) -> Self {
        self.popup = popup;
        self
    }

    pub fn with_region(
        mut self,
        part: SkinPart,
        texture: &str,
        region: (f32, f32, f32, f32),
    ) -> Self {
        self.parts.insert(
            part,
            (
                TextureRef(texture.to_owned()),
                Fit::Region(region.0, region.1, region.2, region.3),
            ),
        );
        self
    }

    pub fn with_check_size(mut self, size: f32) -> Self {
        self.check_size = size;
        self
    }

    pub fn with_box(mut self, dy: f32, text_dx: f32) -> Self {
        self.box_dy = dy;
        self.box_text_dx = text_dx;
        self
    }

    pub fn get(&self, part: SkinPart) -> Option<&TextureRef> {
        self.parts.get(&part).map(|(t, _)| t)
    }

    fn entry(&self, part: SkinPart) -> Option<&(TextureRef, Fit)> {
        self.parts.get(&part)
    }
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
    /// A skin texture over `rect`, stretched or at native size per `fit`.
    Image {
        texture: TextureRef,
        rect: Rect,
        fit: Fit,
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

const FOCUS: Color = Color(255, 214, 90, 255);
const FALLBACK_BOX: Color = Color(120, 70, 20, 255);
const FALLBACK_HOVER: Color = Color(220, 120, 30, 255);
const FALLBACK_TRACK: Color = Color(30, 140, 30, 255);
const FALLBACK_THUMB: Color = Color(200, 140, 40, 255);
const FALLBACK_LIST: Color = Color(70, 40, 12, 255);
const FALLBACK_ROW: Color = Color(220, 120, 30, 255);

fn part(out: &mut Vec<DrawCmd>, skin: &Skin, part: SkinPart, rect: Rect, fallback: Color) {
    match skin.entry(part) {
        Some((texture, fit)) => out.push(DrawCmd::Image {
            texture: texture.clone(),
            rect,
            fit: *fit,
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
pub(crate) fn popup_rect(anchor: Rect, options: usize, skin: &Skin) -> Rect {
    let p = &skin.popup;
    Rect::new(
        anchor.x,
        anchor.bottom(),
        anchor.w,
        p.top + p.bottom + p.row_height * options as f32,
    )
}

/// Where option `index` of a popup is.
pub(crate) fn popup_row(anchor: Rect, index: usize, skin: &Skin) -> Rect {
    let p = &skin.popup;
    Rect::new(
        anchor.x + p.inset,
        anchor.bottom() + p.top + p.row_height * index as f32,
        anchor.w - 2. * p.inset,
        p.row_height,
    )
}

/// Where the slider knob's art is for `value`.
pub(crate) fn slider_thumb(
    rect: Rect,
    min: f32,
    max: f32,
    value: f32,
    metrics: &SliderMetrics,
) -> Rect {
    let frac = if max > min {
        ((value - min) / (max - min)).clamp(0., 1.)
    } else {
        0.
    };
    Rect::new(
        rect.x + metrics.thumb_start + frac * metrics.travel,
        rect.y,
        metrics.thumb_width,
        rect.h,
    )
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

/// Every string the tree draws with its font, so a host can measure them before layout.
pub(crate) fn texts(node: &Node, out: &mut Vec<(String, Font)>) {
    match &node.kind {
        Kind::Text(content) | Kind::Button(content) => out.push((content.clone(), node.style.font)),
        Kind::Checkbox { text, .. } => out.push((text.clone(), node.style.font)),
        Kind::Select { options, .. } => {
            out.extend(options.iter().map(|o| (o.clone(), node.style.font)))
        }
        Kind::NumberInput { .. } | Kind::Slider { .. } | Kind::Box => {}
    }
    for child in &node.children {
        texts(child, out);
    }
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
        Kind::Text(content) => text(
            out,
            rect,
            content,
            style.font,
            style.color,
            style.text_align,
        ),
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
            let size = skin.check_size;
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
            let track = Rect::new(rect.x + skin.slider.track_dx, rect.y, rect.w, rect.h);
            part(out, skin, SkinPart::SliderTrack, track, FALLBACK_TRACK);
            if hover && let Some((texture, fit)) = skin.entry(SkinPart::SliderTrackHover) {
                out.push(DrawCmd::Image {
                    texture: texture.clone(),
                    rect: track,
                    fit: *fit,
                });
            }
            part(
                out,
                skin,
                SkinPart::SliderThumb,
                slider_thumb(rect, *min, *max, *value, &skin.slider),
                FALLBACK_THUMB,
            );
        }
        Kind::Select {
            options, selected, ..
        } => {
            let art = Rect::new(rect.x, rect.y + skin.box_dy, rect.w, rect.h);
            part(
                out,
                skin,
                if hover {
                    SkinPart::BoxHover
                } else {
                    SkinPart::BoxIdle
                },
                art,
                if hover { FALLBACK_HOVER } else { FALLBACK_BOX },
            );
            let value_rect = Rect::new(
                rect.x + skin.box_text_dx,
                rect.y,
                rect.w - skin.box_text_dx - 8.,
                rect.h,
            );
            text(
                out,
                value_rect,
                options.get(*selected).map_or("", String::as_str),
                style.font,
                style.color,
                TextAlign::Left,
            );
            if let Some((_, highlight)) = state.open.filter(|(k, _)| *k == key) {
                part(
                    popups,
                    skin,
                    SkinPart::ListBackground,
                    popup_rect(rect, options.len(), skin),
                    FALLBACK_LIST,
                );
                for (index, option) in options.iter().enumerate() {
                    let row = popup_row(rect, index, skin);
                    if index == highlight {
                        part(popups, skin, SkinPart::ListRowSelected, row, FALLBACK_ROW);
                    }
                    text(
                        popups,
                        Rect::new(
                            row.x + skin.popup.text_dx,
                            row.y,
                            row.w - skin.popup.text_dx - 4.,
                            row.h,
                        ),
                        option,
                        Font::Normal,
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
            let art = Rect::new(rect.x, rect.y + skin.box_dy, rect.w, rect.h);
            part(
                out,
                skin,
                if hover || focused {
                    SkinPart::BoxHover
                } else {
                    SkinPart::BoxIdle
                },
                art,
                if hover || focused {
                    FALLBACK_HOVER
                } else {
                    FALLBACK_BOX
                },
            );
            let editing = state.edit.filter(|(k, _)| *k == key).map(|(_, t)| t);
            let shown = editing.map_or_else(|| format_number(*value, *decimals), str::to_owned);
            let value_rect = Rect::new(
                rect.x + skin.box_text_dx,
                rect.y,
                rect.w - skin.box_text_dx - 8.,
                rect.h,
            );
            text(
                out,
                value_rect,
                &shown,
                style.font,
                style.color,
                TextAlign::Left,
            );
            if editing.is_some() {
                let (tw, th) = metrics.text_size(&shown, style.font);
                out.push(DrawCmd::Rect {
                    rect: Rect::new(value_rect.x + tw + 1., rect.y + (rect.h - th) / 2., 1., th),
                    color: style.color,
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
