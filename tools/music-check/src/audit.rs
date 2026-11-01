//! Role contacts are checked against actual raster ink after a bounding-box rejection.

use gd_engrave::{Engraving, Mark, PositionedMark, Rect, Role, SvgOptions, System};

pub(crate) fn check(drawing: &Engraving) -> Result<(), String> {
    for system in &drawing.systems {
        for (index, a) in system.marks.iter().enumerate() {
            for b in &system.marks[index + 1..] {
                if allowed(a.mark.role(), b.mark.role()) || !a.bounds.overlaps(b.bounds, 0.225) {
                    continue;
                }
                let region = Rect {
                    left: a.bounds.left.max(b.bounds.left),
                    right: a.bounds.right.min(b.bounds.right),
                    top: a.bounds.top.max(b.bounds.top),
                    bottom: a.bounds.bottom.min(b.bounds.bottom),
                };
                let first = mask(drawing, a, region)?;
                let second = mask(drawing, b, region)?;
                let contacts = first
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(second.as_chunks::<4>().0)
                    .filter(|(a, b)| a[3] > 192 && b[3] > 192)
                    .count();
                if contacts > 1 {
                    return Err(format!(
                        "Ink collision: {:?} {:?} with {:?} {:?} ({contacts} pixels)",
                        a.mark.role(),
                        a.bounds,
                        b.mark.role(),
                        b.bounds
                    ));
                }
            }
        }
    }
    Ok(())
}

fn allowed(a: Role, b: Role) -> bool {
    use Role::*;
    if matches!(a, Staff | Bar) || matches!(b, Staff | Bar) {
        return true;
    }
    if a == b
        && matches!(a, Signature | Instrument | Dynamic | Volta | Tuplet | Beam | Ledger | Rest)
    {
        return true;
    }
    matches!(
        (a, b),
        (Notehead, Stem | Ledger)
            | (Stem | Ledger, Notehead)
            | (Stem, Beam | Flag | Ledger)
            | (Beam | Flag | Ledger, Stem)
            | (Flag, Beam)
            | (Beam, Flag)
    )
}

fn mask(drawing: &Engraving, mark: &PositionedMark, region: Rect) -> Result<Vec<u8>, String> {
    let mut glyphs = std::collections::BTreeMap::new();
    if let Mark::Glyph { glyph, .. } = mark.mark {
        glyphs.insert(glyph, drawing.glyphs.get(&glyph).ok_or("Missing audit glyph.")?.clone());
    }
    let drawing = Engraving {
        width: drawing.width,
        height: drawing.height,
        glyphs,
        systems: vec![System {
            width: drawing.width,
            height: drawing.height,
            marks: vec![mark.clone()],
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut svg = String::new();
    gd_engrave::write_svg(&mut svg, &drawing, &SvgOptions::default(), None)
        .map_err(|e| e.to_string())?;
    let start = svg.find("viewBox=\"").ok_or("Missing audit viewport.")? + 9;
    let end = start + svg[start..].find('"').ok_or("Missing audit viewport end.")?;
    let (width, height) = (region.right - region.left, region.bottom - region.top);
    svg.replace_range(start..end, &format!("{} {} {width} {height}", region.left, region.top));
    for (attribute, value) in [("width", width), ("height", height)] {
        let start = svg.find(&format!("{attribute}=\"")).ok_or("Missing audit size.")?
            + attribute.len()
            + 2;
        let end = start + svg[start..].find('"').ok_or("Missing audit size end.")?;
        svg.replace_range(start..end, &format!("{value}pt"));
    }
    let png = gd_typst::rasterize_svg(
        svg.as_bytes(),
        &gd_typst::RasterOptions { width, height, dpi: 288.0, ..Default::default() },
    )
    .map_err(|e| e.to_string())?;
    crate::png::decode(&png).map(|(_, _, pixels)| pixels)
}

pub(crate) fn quants(drawing: &Engraving) -> Result<(), String> {
    for system in &drawing.systems {
        for mark in &system.marks {
            let staff_lines: Vec<_> = system
                .marks
                .iter()
                .filter_map(|line| {
                    if line.staff == mark.staff
                        && let Mark::Line { from, role: Role::Staff, .. } = line.mark
                    {
                        Some(from.y)
                    } else {
                        None
                    }
                })
                .collect();
            if staff_lines.len() != 5 {
                return Err("A staff must have five lines.".into());
            }
            let middle = staff_lines.iter().sum::<f64>() / 5.0;
            if let Mark::Curve { outer, role: Role::Tie, .. } = mark.mark {
                let apex = (outer.point(0.5).y - middle) / 4.5;
                if apex.abs() <= 2.1 && (apex - apex.round()).abs() < 0.14 {
                    return Err(format!("Tie apex touches a staff line: {apex}"));
                }
            }
            beam(system, mark, middle)?;
        }
    }
    Ok(())
}

fn beam(system: &System, mark: &PositionedMark, middle: f64) -> Result<(), String> {
    let Mark::Polygon { points, role: Role::Beam } = mark.mark else { return Ok(()) };
    let secondary = system.marks.iter().any(|other| {
        let Mark::Polygon { points: p, role: Role::Beam } = other.mark else {
            return false;
        };
        if other.staff != mark.staff {
            return false;
        }
        let wider = p[0].x < points[0].x - 0.01 || p[1].x > points[1].x + 0.01;
        let covering = p[0].x <= points[0].x + 0.01 && p[1].x >= points[1].x - 0.01;
        let outer = ((p[0].y + p[1].y) / 2.0 - middle).abs()
            > ((points[0].y + points[1].y) / 2.0 - middle).abs() + 0.01;
        covering && (wider || outer)
    });
    if secondary {
        return Ok(());
    }
    for (a, b) in [(points[0], points[3]), (points[1], points[2])] {
        let joined = system.marks.iter().any(|other| {
            if other.staff != mark.staff {
                return false;
            }
            let Mark::Polygon { points: p, role: Role::Beam } = other.mark else {
                return false;
            };
            [(p[0], points[1]), (p[1], points[0])].iter().any(|(edge, ours)| {
                (ours.x - a.x).abs() < 0.001
                    && (edge.x - a.x).abs() < 0.001
                    && (edge.y - a.y).abs() < 0.001
            })
        });
        if joined {
            continue;
        }
        let values = [a.y, b.y, (a.y + b.y) / 2.0];
        let outside = a.y.min(b.y) > middle + 9.0 || a.y.max(b.y) < middle - 9.0;
        if !outside
            && !values.iter().any(|y| {
                let position = (y - middle) / 4.5;
                (position - position.round()).abs() < 0.027
            })
        {
            return Err(format!(
                "An exposed primary beam end has a forbidden quant: {values:?}, middle {middle}"
            ));
        }
    }
    Ok(())
}
