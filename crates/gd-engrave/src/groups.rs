//! Group furniture follows vertical staff placement so braces span the actual distance.

use gd_score::{Error, GroupKind, StaffId};

use crate::drawing::{Draw, Request};
use crate::limits::charge;
use crate::{Bezier, Mark, Point, Rect, Role};

impl Draw<'_, '_, '_, '_, '_> {
    pub(crate) fn groups(&mut self, request: &Request<'_, '_>) -> Result<(), Error> {
        let space = self.options.staff_size / 4.0;
        if request.score.staves.len() > 1 {
            let last = StaffId((request.score.staves.len() - 1) as u32);
            let top = staff_edge(&self.system.marks, StaffId(0), true);
            let bottom = staff_edge(&self.system.marks, last, false);
            self.staff = StaffId(0);
            self.measure = 0;
            self.line(
                Point { x: request.indent, y: top },
                Point { x: request.indent, y: bottom },
                0.16 * space,
                Role::Bar,
            )?;
        }
        for group in &request.score.groups {
            charge(self.work, self.system.marks.len())?;
            let top = staff_edge(&self.system.marks, group.first, true);
            let bottom = staff_edge(&self.system.marks, group.last, false);
            self.staff = group.first;
            self.measure = 0;
            let first_line = self.first_line(request);
            let label = self.group_label(group, first_line);
            self.instrument_label(label, space, (top + bottom) / 2.0, space)?;
            let x = request.indent - 0.7 * space;
            match group.kind {
                GroupKind::Brace => self.brace(x, top, bottom, space)?,
                GroupKind::Bracket | GroupKind::Choir => {
                    let bx = request.indent - 1.2 * space;
                    self.line(
                        Point { x: bx + 0.225 * space, y: top },
                        Point { x: bx + 0.225 * space, y: bottom },
                        0.45 * space,
                        Role::Instrument,
                    )?;
                    self.glyph("bracketTop", bx, top, space, Role::Instrument)?;
                    self.glyph("bracketBottom", bx, bottom, space, Role::Instrument)?;
                }
            }
            if group.kind != GroupKind::Choir {
                let mut bars = Vec::new();
                for mark in &self.system.marks {
                    charge(self.work, 1)?;
                    if mark.staff == group.first
                        && let Mark::Line { from, to, thickness, role: Role::Bar } = mark.mark
                        && from.x == to.x
                        && (from.x - request.indent).abs() > 0.01
                    {
                        bars.push((from.x, thickness));
                    }
                }
                for (x, thickness) in bars {
                    self.line(Point { x, y: top }, Point { x, y: bottom }, thickness, Role::Bar)?;
                }
            }
        }
        Ok(())
    }

    fn brace(&mut self, x: f64, top: f64, bottom: f64, space: f64) -> Result<(), Error> {
        let middle = (top + bottom) / 2.0;
        for (from, to, side) in [(top, middle, 1.0), (middle, bottom, -1.0)] {
            let length = to - from;
            let outer = Bezier {
                from: Point { x, y: from },
                to: Point { x, y: to },
                first: Point { x: x - side * 1.3 * space, y: from + length * 0.2 },
                second: Point { x: x + side * 0.6 * space, y: to - length * 0.2 },
            };
            let inner = Bezier {
                first: Point { x: outer.first.x + 0.3 * space, ..outer.first },
                second: Point { x: outer.second.x + 0.3 * space, ..outer.second },
                ..outer
            };
            self.add(
                Mark::Curve { outer, inner, role: Role::Instrument },
                Rect { left: x - 1.3 * space, right: x + 0.9 * space, top: from, bottom: to },
            )?;
        }
        Ok(())
    }
}

fn staff_edge(marks: &[crate::PositionedMark], staff: StaffId, top: bool) -> f64 {
    marks
        .iter()
        .filter(|m| m.staff == staff && m.mark.role() == Role::Staff)
        .map(|m| if top { m.bounds.top } else { m.bounds.bottom })
        .reduce(if top { f64::min } else { f64::max })
        .unwrap_or(0.0)
}
