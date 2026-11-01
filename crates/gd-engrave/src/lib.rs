//! Native music engraving: a score becomes positioned marks, then self-contained SVG.
//! Fonts are explicit immutable tables; one reusable Engraver belongs to one thread.

#![forbid(unsafe_code)]

mod beaming;
mod beams;
mod breaking;
mod columns;
mod curves;
mod decorations;
mod drawing;
mod fonts;
mod geometry;
mod heads;
mod layout;
mod limits;
mod marks;
mod math;
mod prepare;
mod rods;
mod signatures;
mod skyline;
mod spacing;
mod svg;
mod text;
mod vertical;

pub use breaking::{BreakPoint, Line, break_lines};
pub use fonts::{FontOptions, Fonts, Glyph, GlyphData, load_fonts};
pub use geometry::{Bezier, Point, Rect};
pub use layout::{Engraver, engrave_svg};
pub use limits::{Limits, Options, Reserve};
pub use marks::{Engraving, Justify, Mark, PositionedMark, Role, Summary, System};
pub use spacing::{Rod, Solution, Spring, solve_spacing};
pub use svg::{SvgOptions, write_svg};
pub use text::{TextGlyph, TextRun};

#[cfg(test)]
mod tests;
