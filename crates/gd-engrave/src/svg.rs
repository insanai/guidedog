use std::collections::BTreeMap;
use std::fmt::{self, Write};
use std::ops::Range;

use gd_score::Error;

use crate::limits::failure;
use crate::{Bezier, Engraving, Mark, PositionedMark, Reserve};

/// Standalone SVG export policy. The default selects every system and black ink.
#[derive(Clone, Debug)]
pub struct SvgOptions<'a> {
    /// Selected system indices, or all systems.
    pub systems: Option<Range<usize>>,
    /// Accessible title, escaped as XML text.
    pub title: &'a str,
    /// Accessible musical description, escaped as XML text.
    pub description: &'a str,
    /// Add screen-only dark-theme ink rules; PDF and PNG use false.
    pub adaptive: bool,
    /// Maximum complete SVG bytes, excluding an existing caller buffer prefix.
    pub max_bytes: usize,
}

impl Default for SvgOptions<'_> {
    fn default() -> Self {
        Self {
            systems: None,
            title: "",
            description: "",
            adaptive: false,
            max_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Appends self-contained SVG to a caller's buffer, borrowing all inputs for this call.
/// Fails on invalid systems or dimensions, allocation refusal or size limits.
/// A failure restores the caller's original buffer length.
pub fn write_svg(
    out: &mut String,
    engraving: &Engraving,
    options: &SvgOptions<'_>,
    reserve: Option<&mut Reserve<'_>>,
) -> Result<(), Error> {
    let range = options.systems.clone().unwrap_or(0..engraving.systems.len());
    let systems = engraving
        .systems
        .get(range)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| failure("Invalid SVG system selection."))?;
    let base = systems[0].top;
    let height = systems.last().map(|s| s.top + s.height - base).unwrap_or(0.0);
    if !engraving.width.is_finite()
        || engraving.width <= 0.0
        || !height.is_finite()
        || height <= 0.0
    {
        return Err(failure("SVG dimensions must be positive and finite."));
    }
    let start = out.len();
    let mut writer = Writer { out, start, limit: options.max_bytes, reserve };
    let result = document(&mut writer, engraving, systems, base, height, options);
    if result.is_err() {
        writer.out.truncate(start);
        return Err(failure("Cannot write SVG within the output or memory limit."));
    }
    Ok(())
}

fn document(
    out: &mut Writer<'_, '_, '_>,
    engraving: &Engraving,
    systems: &[crate::System],
    base: f64,
    height: f64,
    options: &SvgOptions<'_>,
) -> fmt::Result {
    writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}pt\" height=\"{}pt\" \
        viewBox=\"0 0 {} {}\" role=\"img\" color=\"#000\" aria-labelledby=\"title description\">",
        Number(engraving.width),
        Number(height),
        Number(engraving.width),
        Number(height)
    )?;
    writeln!(
        out,
        "<title id=\"title\">{}\u{3c}/title><desc id=\"description\">{}\u{3c}/desc>",
        Xml(options.title),
        Xml(options.description)
    )?;
    if options.adaptive {
        out.write_str(
            "<style>:root{color-scheme:light dark}\
            @media screen and (prefers-color-scheme:dark){:root{color:#e8ecea}}</style>\n",
        )?;
    }
    out.write_str("<defs>\n")?;
    for (glyph, data) in &engraving.glyphs {
        writeln!(out, "<path id=\"music-{}\" d=\"{}\"/>", glyph.0, Xml(&data.outline))?;
    }
    out.write_str("</defs>\n<g fill=\"currentColor\" stroke=\"currentColor\">\n")?;
    for system in systems {
        writeln!(
            out,
            "<g class=\"system\" data-first-measure=\"{}\" \
            transform=\"translate(0 {})\">",
            system.first_measure,
            Number(system.top - base)
        )?;
        let groups = groups(out, &system.marks, engraving)?;
        let mut staff = None;
        for ((staff_id, measure), marks) in groups {
            if staff != Some(staff_id) {
                if staff.is_some() {
                    out.write_str("</g>\n")?;
                }
                writeln!(out, "<g class=\"staff\" data-staff=\"{}\">", staff_id + 1)?;
                staff = Some(staff_id);
            }
            writeln!(out, "<g class=\"measure\" data-measure=\"{measure}\">")?;
            for mark in marks {
                write_mark(out, &mark.mark)?;
            }
            out.write_str("</g>\n")?;
        }
        if staff.is_some() {
            out.write_str("</g>\n")?;
        }
        out.write_str("</g>\n")?;
    }
    out.write_str("</g>\n</svg>\n")
}

fn groups<'a>(
    out: &mut Writer<'_, '_, '_>,
    marks: &'a [PositionedMark],
    engraving: &Engraving,
) -> Result<BTreeMap<(u32, u32), Vec<&'a PositionedMark>>, fmt::Error> {
    let mut groups = BTreeMap::new();
    for mark in marks {
        if let Mark::Glyph { glyph, .. } = mark.mark
            && !engraving.glyphs.contains_key(&glyph)
        {
            return Err(fmt::Error);
        }
        let group: &mut Vec<_> = match groups.entry((mark.staff.0, mark.measure)) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                out.reserve_buffer(192)?;
                slot.insert(Vec::new())
            }
            std::collections::btree_map::Entry::Occupied(slot) => slot.into_mut(),
        };
        if group.len() == group.capacity() {
            let count = group.capacity().max(16);
            out.reserve_buffer(count * std::mem::size_of::<&PositionedMark>())?;
            group.try_reserve_exact(count).map_err(|_| fmt::Error)?;
        }
        group.push(mark);
    }
    Ok(groups)
}

fn write_mark(out: &mut Writer<'_, '_, '_>, mark: &Mark) -> fmt::Result {
    let role = mark.role().name();
    match mark {
        Mark::Glyph { glyph, x, y, size, .. } => {
            if ![x, y, size].iter().all(|v| v.is_finite()) || *size <= 0.0 {
                return Err(fmt::Error);
            }
            writeln!(
                out,
                "<use class=\"{role}\" href=\"#music-{}\" stroke=\"none\" \
                transform=\"translate({} {}) scale({} {})\"/>",
                glyph.0,
                Number(*x),
                Number(*y),
                Number(*size),
                Number(-*size)
            )
        }
        Mark::Line { from, to, thickness, .. } => writeln!(
            out,
            "<path class=\"{role}\" d=\"M{} {}L{} {}\" fill=\"none\" stroke-width=\"{}\"/>",
            Number(from.x),
            Number(from.y),
            Number(to.x),
            Number(to.y),
            Number(*thickness)
        ),
        Mark::Polygon { points, .. } => writeln!(
            out,
            "<path class=\"{role}\" d=\"M{} {}L{} {}L{} {}L{} {}Z\" stroke=\"none\"/>",
            Number(points[0].x),
            Number(points[0].y),
            Number(points[1].x),
            Number(points[1].y),
            Number(points[2].x),
            Number(points[2].y),
            Number(points[3].x),
            Number(points[3].y)
        ),
        Mark::Curve { outer, inner, .. } => {
            write!(out, "<path class=\"{role}\" d=\"")?;
            write_curve(out, *outer, false)?;
            write!(out, "L{} {}", Number(inner.to.x), Number(inner.to.y))?;
            write_curve(out, *inner, true)?;
            out.write_str("Z\" stroke=\"none\"/>\n")
        }
        Mark::Text { run, x, y, .. } => {
            writeln!(out, "<g class=\"{role}\" stroke=\"none\">")?;
            for glyph in &run.glyphs {
                writeln!(
                    out,
                    "<path d=\"{}\" transform=\"translate({} {}) scale(1 -1)\"/>",
                    Xml(&glyph.outline),
                    Number(x + glyph.x),
                    Number(y + glyph.y)
                )?;
            }
            out.write_str("</g>\n")
        }
    }
}

fn write_curve(out: &mut Writer<'_, '_, '_>, curve: Bezier, reverse: bool) -> fmt::Result {
    if reverse {
        write!(
            out,
            "C{} {} {} {} {} {}",
            Number(curve.second.x),
            Number(curve.second.y),
            Number(curve.first.x),
            Number(curve.first.y),
            Number(curve.from.x),
            Number(curve.from.y)
        )
    } else {
        write!(
            out,
            "M{} {}C{} {} {} {} {} {}",
            Number(curve.from.x),
            Number(curve.from.y),
            Number(curve.first.x),
            Number(curve.first.y),
            Number(curve.second.x),
            Number(curve.second.y),
            Number(curve.to.x),
            Number(curve.to.y)
        )
    }
}

struct Writer<'s, 'r, 'callback> {
    out: &'s mut String,
    start: usize,
    limit: usize,
    reserve: Option<&'r mut Reserve<'callback>>,
}

impl Writer<'_, '_, '_> {
    fn reserve_buffer(&mut self, bytes: usize) -> fmt::Result {
        if self.reserve.as_mut().is_some_and(|reserve| !reserve(bytes)) {
            return Err(fmt::Error);
        }
        Ok(())
    }
}

impl Write for Writer<'_, '_, '_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let needed = self.out.len().checked_add(text.len()).ok_or(fmt::Error)?;
        if needed - self.start > self.limit {
            return Err(fmt::Error);
        }
        if needed > self.out.capacity() {
            let capacity = needed.checked_next_power_of_two().ok_or(fmt::Error)?.max(4096);
            let growth = capacity - self.out.capacity();
            if self.reserve.as_mut().is_some_and(|reserve| !reserve(growth)) {
                return Err(fmt::Error);
            }
            self.out.try_reserve_exact(capacity - self.out.len()).map_err(|_| fmt::Error)?;
        }
        self.out.push_str(text);
        Ok(())
    }
}

pub(crate) struct Number(pub(crate) f64);

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.0.is_finite() {
            return Err(fmt::Error);
        }
        let text = format!("{:.3}", self.0);
        let text = text.trim_end_matches('0').trim_end_matches('.');
        f.write_str(if text.is_empty() || text == "-0" { "0" } else { text })
    }
}

struct Xml<'a>(&'a str);

impl fmt::Display for Xml<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in self.0.chars() {
            match c {
                '&' => f.write_str("&amp;")?,
                '<' => f.write_str("&lt;")?,
                '>' => f.write_str("&gt;")?,
                '"' => f.write_str("&quot;")?,
                '\'' => f.write_str("&apos;")?,
                c if c >= ' ' || matches!(c, '\n' | '\r' | '\t') => f.write_char(c)?,
                _ => f.write_str("&#xfffd;")?,
            }
        }
        Ok(())
    }
}
