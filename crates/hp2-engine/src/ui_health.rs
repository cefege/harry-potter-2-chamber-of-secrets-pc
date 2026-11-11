//! Source-derived, live-read policy for `StatusItemHealth.DrawItem`.
//!
//! Not connected to the HUD. The adapter must execute operations synchronously.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Field {
    Count,
    Potential,
    UnitsPerIcon,
    ActualIconWidth,
    ActualIconHeight,
    /// Read the current `textureHudIcon` object, then its `USize`.
    TextureWidth,
    /// Read the current `textureHudIcon` object, then its `VSize`.
    TextureHeight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Texture {
    /// Resolve the current `textureHealthBlack` when drawing.
    Black,
    /// Resolve the current `textureHealthBorder` when drawing.
    Border,
    /// Resolve the current `textureHealthOrangeInside` when drawing.
    Orange,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Op {
    SaveStyle,
    SaveDrawColor,
    /// Set R, G and B to 255 without changing A.
    WhiteRgb,
    StyleBlack,
    StyleSaved,
    SetPos(f32, f32),
    DrawIcon(Texture, f32),
    /// Dispatch the active state's `GetHealthDrawColor` and assign all four bytes.
    HealthDrawColor,
    /// Evaluate the texture argument before the tile's dimension arguments.
    ResolveTileTexture(Texture),
    DrawTile {
        texture: Texture,
        width: f32,
        height: f32,
        u: f32,
        v: f32,
        ul: f32,
        vl: f32,
    },
    RestoreDrawColor,
}

/// Reads and effects occur in source order; implementations must not cache fields
/// across calls or hold incompatible borrows across `HealthDrawColor`.
pub(crate) trait HealthAdapter {
    type Error;
    fn read(&mut self, field: Field) -> Result<i32, Self::Error>;
    fn normal_display(&mut self) -> Result<bool, Self::Error>;
    fn apply(&mut self, op: Op) -> Result<(), Self::Error>;
}

// Object.uc declares `int += (out int, int)` as native 161. Its RHS is
// FloatToInt before the VM's wrapping integer addition; this is not float +=.
fn advance_x(x: i32, width: i32, scale: f32) -> i32 {
    x.wrapping_add((width as f32 * scale) as i32)
}

/// Runs the drawing statements; scale is a by-value script argument.
/// Canvas cursor and style are not restored, and errors stop at their first effect.
pub(crate) fn draw_health<A: HealthAdapter>(adapter: &mut A, scale: f32) -> Result<(), A::Error> {
    let mut x = (5.0 * scale) as i32;
    let potential = adapter.read(Field::Potential)?;
    let units = adapter.read(Field::UnitsPerIcon)?;
    let mut icons = if units == 0 {
        0
    } else {
        potential.wrapping_div(units)
    };
    let potential = adapter.read(Field::Potential)? as f32;
    let units = adapter.read(Field::UnitsPerIcon)? as f32;
    // UnrealScript's `%` invokes FloatFloat with a double-precision fmod.
    if ((potential as f64 % units as f64) as f32) > 0.0 {
        icons = icons.wrapping_add(1);
    }
    let mut remaining = adapter.read(Field::Count)?;
    adapter.apply(Op::SaveStyle)?;
    adapter.apply(Op::SaveDrawColor)?;
    for i in 0..icons {
        let y = if i < 3 {
            (5.0 * scale) as i32
        } else {
            (5_i32
                .wrapping_add(adapter.read(Field::ActualIconHeight)?)
                .wrapping_add(4) as f32
                * scale) as i32
        };
        if i == 3 {
            x = (5.0 * scale) as i32;
        }
        let units = adapter.read(Field::UnitsPerIcon)?;
        let fill = if remaining >= units {
            1.0
        } else {
            remaining as f32 / adapter.read(Field::UnitsPerIcon)? as f32
        };
        remaining = remaining.wrapping_sub(adapter.read(Field::UnitsPerIcon)?);
        if remaining < 0 {
            remaining = 0;
        }
        adapter.apply(Op::WhiteRgb)?;
        adapter.apply(Op::StyleBlack)?;
        adapter.apply(Op::SetPos(x as f32, y as f32))?;
        adapter.apply(Op::DrawIcon(Texture::Black, scale))?;
        adapter.apply(Op::StyleSaved)?;
        adapter.apply(Op::SetPos(x as f32, y as f32))?;
        adapter.apply(Op::DrawIcon(Texture::Border, scale))?;
        if !adapter.normal_display()? {
            let after =
                adapter.read(Field::Count)? as f32 / adapter.read(Field::UnitsPerIcon)? as f32;
            if after >= i as f32 && after <= i.wrapping_add(1) as f32 {
                adapter.apply(Op::HealthDrawColor)?;
            }
        }
        if fill > 0.0 {
            let height = fill * adapter.read(Field::TextureHeight)?.wrapping_sub(13) as f32;
            let start = adapter.read(Field::TextureHeight)?.wrapping_sub(11) as f32 - height;
            let height = height + 11.0;
            adapter.apply(Op::SetPos(x as f32, y as f32 + start * scale))?;
            adapter.apply(Op::ResolveTileTexture(Texture::Orange))?;
            let width = adapter.read(Field::TextureWidth)? as f32 * scale;
            let ul = adapter.read(Field::TextureWidth)? as f32;
            adapter.apply(Op::DrawTile {
                texture: Texture::Orange,
                width,
                height: height * scale,
                u: 0.0,
                v: start,
                ul,
                vl: height,
            })?;
        }
        x = advance_x(x, adapter.read(Field::ActualIconWidth)?, scale);
    }
    adapter.apply(Op::RestoreDrawColor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    enum Event {
        Read(Field),
        State,
        Op(Op),
        Draw(Texture, i32),
        Color([u8; 4]),
    }
    struct Mock {
        count: i32,
        potential: i32,
        units: i32,
        width: i32,
        height: i32,
        tw: i32,
        th: i32,
        texture: i32,
        resolved_texture: i32,
        normal: bool,
        color: [u8; 4],
        events: Vec<Event>,
        callbacks: usize,
        mutate: bool,
        fail_color: bool,
    }
    impl Mock {
        fn new(count: i32, potential: i32) -> Self {
            Self {
                count,
                potential,
                units: 100,
                width: 21,
                height: 58,
                tw: 128,
                th: 128,
                texture: 1,
                resolved_texture: 0,
                normal: true,
                color: [255, 0, 0, 0],
                events: Vec::new(),
                callbacks: 0,
                mutate: false,
                fail_color: false,
            }
        }
        fn ops(&self) -> Vec<Op> {
            self.events
                .iter()
                .filter_map(|e| {
                    if let Event::Op(op) = e {
                        Some(*op)
                    } else {
                        None
                    }
                })
                .collect()
        }
        fn tiles(&self) -> Vec<(f32, f32, f32)> {
            self.ops()
                .iter()
                .filter_map(|op| {
                    if let Op::DrawTile { width, v, vl, .. } = *op {
                        Some((width, v, vl))
                    } else {
                        None
                    }
                })
                .collect()
        }
    }
    impl HealthAdapter for Mock {
        type Error = &'static str;
        fn read(&mut self, field: Field) -> Result<i32, Self::Error> {
            self.events.push(Event::Read(field));
            Ok(match field {
                Field::Count => self.count,
                Field::Potential => self.potential,
                Field::UnitsPerIcon => self.units,
                Field::ActualIconWidth => self.width,
                Field::ActualIconHeight => self.height,
                Field::TextureWidth => self.tw,
                Field::TextureHeight => self.th,
            })
        }
        fn normal_display(&mut self) -> Result<bool, Self::Error> {
            self.events.push(Event::State);
            Ok(self.normal)
        }
        fn apply(&mut self, op: Op) -> Result<(), Self::Error> {
            self.events.push(Event::Op(op));
            match op {
                Op::HealthDrawColor => {
                    self.callbacks += 1;
                    if self.fail_color {
                        return Err("color");
                    }
                    self.events.push(Event::Color(self.color));
                    if self.mutate {
                        self.count = 50;
                        self.units = 50;
                        self.texture = 2;
                        self.tw = 64;
                        self.th = 64;
                        self.normal = true;
                    }
                }
                Op::ResolveTileTexture(_) => self.resolved_texture = self.texture,
                Op::DrawIcon(texture, _) => self.events.push(Event::Draw(texture, self.texture)),
                Op::DrawTile { texture, .. } => self
                    .events
                    .push(Event::Draw(texture, self.resolved_texture)),
                _ => {}
            }
            Ok(())
        }
    }
    fn run(count: i32, potential: i32) -> Mock {
        let mut m = Mock::new(count, potential);
        draw_health(&mut m, 1.0).unwrap();
        m
    }
    #[test]
    fn empty_and_partial_full_geometry() {
        assert_eq!(
            run(0, 0).ops(),
            [Op::SaveStyle, Op::SaveDrawColor, Op::RestoreDrawColor]
        );
        assert!(run(0, 100).tiles().is_empty());
        assert_eq!(run(50, 100).tiles(), [(128., 59.5, 68.5)]);
        assert_eq!(run(100, 100).tiles(), [(128., 2., 126.)]);
        assert_eq!(
            run(125, 201).tiles(),
            [(128., 2., 126.), (128., 88.25, 39.75)]
        );
    }
    #[test]
    fn canvas_order_and_cursor_are_not_restored() {
        let ops = run(0, 100).ops();
        assert_eq!(
            ops,
            [
                Op::SaveStyle,
                Op::SaveDrawColor,
                Op::WhiteRgb,
                Op::StyleBlack,
                Op::SetPos(5., 5.),
                Op::DrawIcon(Texture::Black, 1.),
                Op::StyleSaved,
                Op::SetPos(5., 5.),
                Op::DrawIcon(Texture::Border, 1.),
                Op::RestoreDrawColor
            ]
        );
        let ops = run(50, 100).ops();
        assert_eq!(ops[ops.len() - 4], Op::SetPos(5., 64.5));
        assert_eq!(ops[ops.len() - 3], Op::ResolveTileTexture(Texture::Orange));
        assert!(matches!(ops[ops.len() - 2], Op::DrawTile { .. }));
    }
    #[test]
    fn fractional_scale_multirow_truncates_each_step() {
        let mut m = Mock::new(400, 400);
        draw_health(&mut m, 0.75).unwrap();
        let p: Vec<_> = m
            .ops()
            .into_iter()
            .filter_map(|op| match op {
                Op::SetPos(x, y) if y == 3. || y == 50. => Some((x, y)),
                _ => None,
            })
            .collect();
        assert_eq!(
            p,
            [
                (3., 3.),
                (3., 3.),
                (18., 3.),
                (18., 3.),
                (33., 3.),
                (33., 3.),
                (3., 50.),
                (3., 50.)
            ]
        );
    }
    #[test]
    fn inclusive_boundaries_query_both_icons() {
        for (count, expected) in [(0, 1), (50, 1), (100, 2), (200, 2), (300, 1)] {
            let mut m = Mock::new(count, 300);
            m.normal = false;
            draw_health(&mut m, 1.).unwrap();
            assert_eq!(m.callbacks, expected, "count {count}");
        }
    }
    #[test]
    fn damage_heal_and_fade_colors_are_not_computed_by_policy() {
        for color in [
            [255, 0, 0, 0],
            [0, 255, 0, 0],
            [255, 127, 127, 0],
            [127, 255, 127, 0],
        ] {
            let mut m = Mock::new(100, 200);
            m.normal = false;
            m.color = color;
            draw_health(&mut m, 1.).unwrap();
            assert_eq!(m.callbacks, 2);
            assert_eq!(
                m.events
                    .iter()
                    .filter(|event| **event == Event::Color(color))
                    .count(),
                2
            );
        }
    }
    #[test]
    fn callback_mutations_are_read_after_reentry() {
        let mut m = Mock::new(100, 300);
        m.normal = false;
        m.mutate = true;
        draw_health(&mut m, 1.).unwrap();
        assert_eq!(m.callbacks, 1); // The callback switched to NormalDisplay.
        assert_eq!(m.tiles(), [(64., 2., 62.)]); // New HUD texture size; fill was already local.
        let color = m
            .events
            .iter()
            .position(|e| *e == Event::Op(Op::HealthDrawColor))
            .unwrap();
        assert_eq!(
            &m.events[color + 1..color + 7],
            &[
                Event::Color([255, 0, 0, 0]),
                Event::Read(Field::TextureHeight),
                Event::Read(Field::TextureHeight),
                Event::Op(Op::SetPos(5., 7.)),
                Event::Op(Op::ResolveTileTexture(Texture::Orange)),
                Event::Read(Field::TextureWidth)
            ]
        );
        assert!(m.events[color + 1..].contains(&Event::Draw(Texture::Orange, 2)));
        // The remaining count was computed before the callback; new units apply on later icons.
        assert_eq!(
            m.events
                .iter()
                .filter(|e| **e == Event::Read(Field::UnitsPerIcon))
                .count(),
            11
        );
    }
    #[test]
    fn callback_count_and_units_change_are_observed_on_next_icon() {
        let mut m = Mock::new(150, 300);
        m.normal = false;
        // After the first affected callback, keep the non-normal state but mutate count and units.
        struct Wrap(Mock);
        impl HealthAdapter for Wrap {
            type Error = &'static str;
            fn read(&mut self, f: Field) -> Result<i32, Self::Error> {
                self.0.read(f)
            }
            fn normal_display(&mut self) -> Result<bool, Self::Error> {
                self.0.normal_display()
            }
            fn apply(&mut self, op: Op) -> Result<(), Self::Error> {
                self.0.apply(op)?;
                if op == Op::HealthDrawColor {
                    self.0.count = 0;
                    self.0.units = 50;
                }
                Ok(())
            }
        }
        let mut w = Wrap(m);
        draw_health(&mut w, 1.).unwrap();
        assert_eq!(w.0.callbacks, 1);
        assert_eq!(w.0.tiles(), [(128., 2., 126.), (128., 59.5, 68.5)]);
    }
    #[test]
    fn integer_compound_add_casts_rhs_before_wrapping_addition() {
        assert_eq!(advance_x(5, -1, 0.75), 5);
        assert_eq!(advance_x(-5, 1, 0.75), -5);
        assert_eq!(advance_x(i32::MAX, 1, 1.0), i32::MIN);
        assert_eq!(advance_x(16_777_217, 2, 1.0), 16_777_219);
        assert_eq!(advance_x(4, 1, f32::NAN), 4);
        assert_eq!(advance_x(4, 1, f32::INFINITY), i32::MIN + 3);
    }
    #[test]
    fn zero_divisor_and_negative_remainder_match_vm_icon_count() {
        let mut zero = Mock::new(10, 100);
        zero.units = 0;
        draw_health(&mut zero, 1.).unwrap();
        assert_eq!(
            zero.ops(),
            [Op::SaveStyle, Op::SaveDrawColor, Op::RestoreDrawColor]
        );
        assert_eq!(
            run(100, -50).ops(),
            [Op::SaveStyle, Op::SaveDrawColor, Op::RestoreDrawColor]
        );
        assert_eq!(
            run(0, 101)
                .ops()
                .iter()
                .filter(|op| **op == Op::StyleBlack)
                .count(),
            2
        );
    }
    #[test]
    fn failed_callback_does_not_restore_or_draw_segment() {
        let mut m = Mock::new(50, 100);
        m.normal = false;
        m.fail_color = true;
        assert_eq!(draw_health(&mut m, 1.), Err("color"));
        assert_eq!(m.ops().last(), Some(&Op::HealthDrawColor));
    }
}
