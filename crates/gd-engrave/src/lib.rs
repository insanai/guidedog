//! Native music engraving: a score becomes positioned marks, then self-contained SVG.
//! Fonts are explicit immutable tables; one reusable Engraver belongs to one thread.

#![forbid(unsafe_code)]

mod accidentals;
mod bars;
mod beam_segments;
mod beaming;
mod beams;
mod bows;
mod breaking;
mod columns;
mod courtesy;
mod curves;
mod decorations;
mod dots;
mod drawing;
mod fonts;
mod footprints;
mod geometry;
mod groups;
mod hairpins;
mod heads;
mod labels;
mod layout;
mod limits;
mod lyrics;
mod marks;
mod markup;
mod math;
mod memory;
mod multirests;
mod prepare;
mod rests;
mod rods;
mod scratch;
mod signatures;
mod skyline;
mod spacing;
mod svg;
mod text;
mod tuplets;
mod vertical;
mod voices;

pub use breaking::{BreakPoint, Line, break_lines};
pub use fonts::{FaceData, FaceSource, FontOptions, Fonts, Glyph, GlyphData, load_fonts};
pub use geometry::{Bezier, Point, Rect};
pub use layout::{Engraver, engrave_svg};
pub use limits::{Limits, Options, Reserve};
pub use marks::{Engraving, Justify, Mark, PositionedMark, Role, Summary, System};
pub use scratch::Scratch;
pub use spacing::{Rod, Solution, Spring, solve_spacing};
pub use svg::{SvgOptions, write_svg};
pub use text::{TextGlyph, TextRun};

#[cfg(test)]
mod tests;
