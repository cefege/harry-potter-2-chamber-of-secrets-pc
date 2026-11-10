//! Flexbox-style layout: rows and columns, gaps, padding, fixed/auto/grow sizes, alignment.
use crate::{
    node::{Kind, Node},
    style::{Align, Direction, Font, Justify, Rect, Size},
};

/// Measures text for the layout. The host implements it with its real fonts.
pub trait TextMetrics {
    /// Width and height of `text` in `font`.
    fn text_size(&self, text: &str, font: Font) -> (f32, f32);
}

/// Monospace metrics for tests and headless use.
#[derive(Clone, Copy, Debug)]
pub struct FixedMetrics {
    pub char_width: f32,
    pub line_height: f32,
}

impl Default for FixedMetrics {
    fn default() -> Self {
        Self {
            char_width: 7.,
            line_height: 14.,
        }
    }
}

impl TextMetrics for FixedMetrics {
    fn text_size(&self, text: &str, _font: Font) -> (f32, f32) {
        (
            text.chars().count() as f32 * self.char_width,
            self.line_height,
        )
    }
}

/// Natural sizes of the built-in widgets (the stock menu art's dimensions).
pub(crate) const WIDGET_HEIGHT: f32 = 24.;
pub(crate) const SLIDER_WIDTH: f32 = 133.;
pub(crate) const BOX_WIDTH: f32 = 180.;
pub(crate) const CHECK_SIZE: f32 = 16.;
pub(crate) const CHECK_GAP: f32 = 6.;
pub(crate) const BUTTON_PAD: f32 = 12.;

fn intrinsic(node: &Node, m: &dyn TextMetrics) -> (f32, f32) {
    let pad = node.style.padding;
    let (w, h) = match &node.kind {
        Kind::Text(text) => m.text_size(text, node.style.font),
        Kind::Button(text) => {
            let (tw, th) = m.text_size(text, node.style.font);
            (tw + 2. * BUTTON_PAD, th.max(WIDGET_HEIGHT))
        }
        Kind::Checkbox { text, .. } => {
            let (tw, th) = m.text_size(text, node.style.font);
            (CHECK_SIZE + CHECK_GAP + tw, th.max(CHECK_SIZE))
        }
        Kind::Slider { .. } => (SLIDER_WIDTH, WIDGET_HEIGHT),
        Kind::Select { .. } | Kind::NumberInput { .. } => (BOX_WIDTH, WIDGET_HEIGHT),
        Kind::Box => {
            let flow: Vec<(f32, f32)> = node
                .children
                .iter()
                .filter(|c| c.style.absolute.is_none())
                .map(|c| outer(c, m))
                .collect();
            let gaps = node.style.gap * flow.len().saturating_sub(1) as f32;
            match node.style.direction {
                Direction::Row => (
                    flow.iter().map(|s| s.0).sum::<f32>() + gaps,
                    flow.iter().map(|s| s.1).fold(0., f32::max),
                ),
                Direction::Column => (
                    flow.iter().map(|s| s.0).fold(0., f32::max),
                    flow.iter().map(|s| s.1).sum::<f32>() + gaps,
                ),
            }
        }
    };
    (w + pad.horizontal(), h + pad.vertical())
}

/// Size of the node's box after its own `width`/`height`. A `Grow` axis measures as its natural
/// size here; the parent then redistributes the free space.
fn outer(node: &Node, m: &dyn TextMetrics) -> (f32, f32) {
    let (iw, ih) = intrinsic(node, m);
    let w = match node.style.width {
        Size::Px(v) => v,
        Size::Auto | Size::Grow(_) => iw,
    };
    let h = match node.style.height {
        Size::Px(v) => v,
        Size::Auto | Size::Grow(_) => ih,
    };
    (w, h)
}

/// Lays `node` out inside `bounds` and records every descendant's rectangle.
pub(crate) fn layout(node: &mut Node, bounds: Rect, m: &dyn TextMetrics) {
    node.rect = bounds;
    if !matches!(node.kind, Kind::Box) {
        return;
    }
    let pad = node.style.padding;
    let content = Rect::new(
        bounds.x + pad.left,
        bounds.y + pad.top,
        (bounds.w - pad.horizontal()).max(0.),
        (bounds.h - pad.vertical()).max(0.),
    );
    let row = node.style.direction == Direction::Row;
    let (main_total, cross_total) = if row {
        (content.w, content.h)
    } else {
        (content.h, content.w)
    };
    let gap = node.style.gap;
    let align = node.style.align;
    let justify = node.style.justify;

    // Absolute children sit at their own offset and never take part in the flow.
    for child in node
        .children
        .iter_mut()
        .filter(|c| c.style.absolute.is_some())
    {
        let (ax, ay) = child.style.absolute.unwrap_or_default();
        let (w, h) = outer(child, m);
        layout(child, Rect::new(content.x + ax, content.y + ay, w, h), m);
    }

    let flow: Vec<usize> = (0..node.children.len())
        .filter(|&i| node.children[i].style.absolute.is_none())
        .collect();
    let sizes: Vec<(f32, f32)> = flow.iter().map(|&i| outer(&node.children[i], m)).collect();
    let main_of = |size: (f32, f32)| if row { size.0 } else { size.1 };
    let main_style = |c: &Node| if row { c.style.width } else { c.style.height };
    let cross_style = |c: &Node| if row { c.style.height } else { c.style.width };

    let gaps = gap * flow.len().saturating_sub(1) as f32;
    let fixed: f32 = flow
        .iter()
        .zip(&sizes)
        .filter(|(i, _)| !matches!(main_style(&node.children[**i]), Size::Grow(_)))
        .map(|(_, s)| main_of(*s))
        .sum();
    let weight: f32 = flow
        .iter()
        .map(|&i| {
            if let Size::Grow(w) = main_style(&node.children[i]) {
                w
            } else {
                0.
            }
        })
        .sum();
    let free = (main_total - fixed - gaps).max(0.);

    let mains: Vec<f32> = flow
        .iter()
        .zip(&sizes)
        .map(|(&i, s)| match main_style(&node.children[i]) {
            Size::Grow(w) if weight > 0. => free * w / weight,
            _ => main_of(*s),
        })
        .collect();
    let used: f32 = mains.iter().sum::<f32>() + gaps;
    let leftover = (main_total - used).max(0.);
    let (mut cursor, between) = if weight > 0. {
        (0., gap)
    } else {
        match justify {
            Justify::Start => (0., gap),
            Justify::Center => (leftover / 2., gap),
            Justify::End => (leftover, gap),
            Justify::SpaceBetween if flow.len() > 1 => {
                (0., gap + leftover / (flow.len() - 1) as f32)
            }
            Justify::SpaceBetween => (0., gap),
        }
    };

    for (slot, &i) in flow.iter().enumerate() {
        let size = sizes[slot];
        let main = mains[slot];
        let cross_natural = if row { size.1 } else { size.0 };
        let cross = match (cross_style(&node.children[i]), align) {
            (Size::Px(v), _) => v,
            (_, Align::Stretch) => cross_total,
            _ => cross_natural,
        };
        let cross_offset = match align {
            Align::Start | Align::Stretch => 0.,
            Align::Center => (cross_total - cross) / 2.,
            Align::End => cross_total - cross,
        };
        let rect = if row {
            Rect::new(content.x + cursor, content.y + cross_offset, main, cross)
        } else {
            Rect::new(content.x + cross_offset, content.y + cursor, cross, main)
        };
        layout(&mut node.children[i], rect, m);
        cursor += main + between;
    }
}

/// Lays a root out at `(0, 0)` sized to the viewport.
pub(crate) fn layout_root(root: &mut Node, width: f32, height: f32, m: &dyn TextMetrics) {
    layout(root, Rect::new(0., 0., width, height), m);
}
