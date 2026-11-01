use gd_score::{Error, EventKind, Score};

use crate::columns::{self, Column};
use crate::limits::{failure, push};
use crate::{BreakPoint, Engraving, Fonts, Options, Spring, Summary, SvgOptions};

/// Reusable per-thread layout buffers. No process-global state or hidden font policy.
#[derive(Debug, Default)]
pub struct Engraver {
    pub(crate) columns: Vec<Column>,
    pub(crate) springs: Vec<Spring>,
    pub(crate) breaks: Vec<BreakPoint>,
    pub(crate) positions: Vec<f64>,
    pub(crate) plan: crate::prepare::Plan,
    pub(crate) rods: Vec<crate::Rod>,
    local_rods: Vec<crate::Rod>,
}

impl Engraver {
    /// Borrows a score and immutable fonts and produces owned systems of geometry.
    /// Fails for invalid notation, dimensions, fonts, work bounds or memory refusal.
    pub fn engrave_score(
        &mut self,
        score: &Score,
        fonts: &Fonts<'_>,
        options: &mut Options<'_>,
    ) -> Result<Engraving, Error> {
        validate(score, options)?;
        let mut work = options.limits.work;
        columns::collect(&mut self.columns, score, options, &mut work)?;
        self.plan.load(score, &self.columns, fonts, options, &mut work)?;
        columns::springs(&mut self.springs, &self.columns, score, options, &mut work)?;
        columns::breaks(
            &mut self.breaks,
            &self.columns,
            score,
            &self.plan,
            fonts,
            options,
            &mut work,
        )?;
        crate::rods::collect(
            &mut self.rods,
            &self.columns,
            &self.plan,
            fonts,
            options,
            &mut work,
        )?;
        let space = options.staff_size / 4.0;
        let lines = crate::breaking::choose(
            &self.springs,
            &self.rods,
            &self.breaks,
            (options.line_width / space - 1.0).max(0.1),
            options.justify,
            &options.limits,
            &mut work,
        )?;
        let mut result = Engraving { summary: summary(score), ..Engraving::default() };
        let mut top = 0.0;
        for line in &lines {
            let (first, last) = (self.breaks[line.first].gap, self.breaks[line.last].gap);
            let x = self.line_positions(line, space, options, &mut work)?;
            let request = crate::drawing::Request {
                score,
                fonts,
                columns: &self.columns[first..=last],
                positions: &self.positions,
                first: self.breaks[line.first].measure,
                plan: &self.plan,
                indent: self.breaks[line.first].indent * space,
            };
            let (mut system, warnings) =
                crate::drawing::draw_system(&request, options, &mut work)?;
            result.warnings.extend(warnings);
            system.top = top;
            top += system.height + 2.0 * space;
            if top > options.limits.height {
                return Err(failure("The drawing exceeds its height limit."));
            }
            result.width = result.width.max(x + space);
            if line.overfull {
                result.warnings.push(
                    gd_score::Error::new(
                        gd_score::ErrorKind::Overfull,
                        format!("Measure {} exceeds the available width.", system.first_measure),
                    )
                    .warning()
                    .hint("Widen the music or split the measure into a shorter example."),
                );
            }
            push(&mut result.systems, system, &mut options.reserve)?;
        }
        result.height = (top - 2.0 * space).max(space);
        definitions(&mut result, fonts, options)?;
        Ok(result)
    }
    fn line_positions(
        &mut self,
        line: &crate::Line,
        space: f64,
        options: &mut Options<'_>,
        work: &mut usize,
    ) -> Result<f64, Error> {
        let (first, last) = (self.breaks[line.first].gap, self.breaks[line.last].gap);
        self.local_rods.clear();
        for rod in &self.rods {
            crate::limits::charge(work, 1)?;
            if rod.first >= first && rod.last <= last {
                push(
                    &mut self.local_rods,
                    crate::Rod {
                        first: rod.first - first,
                        last: rod.last - first,
                        minimum: rod.minimum,
                    },
                    &mut options.reserve,
                )?;
            }
        }
        let prefix = self.breaks[line.first].prefix;
        let solution = crate::spacing::solve(
            &self.springs[first..last],
            &self.local_rods,
            (line.width - prefix - self.breaks[line.last].suffix).max(0.01),
            work,
        )?;
        self.positions.clear();
        let mut x = prefix * space;
        push(&mut self.positions, x, &mut options.reserve)?;
        for gap in solution.gaps {
            x += gap * space;
            push(&mut self.positions, x, &mut options.reserve)?;
        }
        Ok(x)
    }
}

/// One-call SVG engraving with fresh workspace buffers and default non-adaptive export.
/// Borrows score, fonts and options; fails like Engraver::engrave_score or write_svg.
pub fn engrave_svg(
    score: &Score,
    fonts: &Fonts<'_>,
    options: &mut Options<'_>,
) -> Result<String, Error> {
    let engraving = Engraver::default().engrave_score(score, fonts, options)?;
    let mut out = String::new();
    let svg = SvgOptions { max_bytes: options.limits.svg_bytes, ..SvgOptions::default() };
    crate::write_svg(&mut out, &engraving, &svg, options.reserve.as_deref_mut())?;
    Ok(out)
}

fn definitions(
    result: &mut Engraving,
    fonts: &Fonts<'_>,
    options: &mut Options<'_>,
) -> Result<(), Error> {
    for system in &result.systems {
        for placed in &system.marks {
            if let crate::Mark::Glyph { glyph, .. } = placed.mark
                && !result.glyphs.contains_key(&glyph)
            {
                let data =
                    fonts.glyph_data(glyph).ok_or_else(|| failure("Unknown music glyph."))?;
                if options.reserve.as_mut().is_some_and(|reserve| !reserve(data.outline.len())) {
                    return Err(failure("The memory ledger refused music glyph definitions."));
                }
                result.glyphs.insert(glyph, data.clone());
            }
        }
    }
    Ok(())
}

fn validate(score: &Score, options: &Options<'_>) -> Result<(), Error> {
    if !options.line_width.is_finite()
        || options.line_width <= 0.0
        || options.line_width > 100_000.0
        || !options.staff_size.is_finite()
        || !(10.0..=40.0).contains(&options.staff_size)
        || score.events.len() > options.limits.events
        || score.measures.is_empty()
    {
        return Err(failure("Invalid music dimensions or event count."));
    }
    gd_score::check_score(score)
}

fn summary(score: &Score) -> Summary {
    Summary {
        staves: score.staves.len() as u32,
        instruments: score
            .staves
            .iter()
            .filter(|s| !s.name.is_empty())
            .map(|s| s.name.clone())
            .collect(),
        key: score
            .events
            .iter()
            .find_map(|e| if let EventKind::Key(k) = e.kind { Some(k) } else { None }),
        meter: score.measures.first().map(|m| m.meter.clone()),
        measures: (
            score.measures.first().map(|m| m.number).unwrap_or(1),
            score.measures.last().map(|m| m.number).unwrap_or(1),
        ),
        lyrics: score.lyrics.iter().any(|line| !line.syllables.is_empty()),
    }
}
