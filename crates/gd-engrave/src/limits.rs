use std::fmt;

use gd_score::{Error, ErrorKind, Span};

use crate::Justify;

/// A caller-owned memory ledger, consulted before each buffer grows.
pub type Reserve<'a> = dyn FnMut(usize) -> bool + 'a;

/// Explicit bounds on layout work, output size and system count.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Maximum input event count.
    pub events: usize,
    /// Maximum line-breaking candidate window in measures.
    pub measures_per_line: usize,
    /// Maximum systems in one drawing.
    pub systems: usize,
    /// Maximum height in points.
    pub height: f64,
    /// Charged candidate gaps, comparisons and curve samples.
    pub work: usize,
    /// Maximum positioned marks across every system.
    pub marks: usize,
    /// Maximum serialized SVG bytes, including outlines.
    pub svg_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            events: 20_000,
            measures_per_line: 128,
            systems: 64,
            height: 20_000.0,
            work: 8_000_000,
            marks: 500_000,
            svg_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Plain layout options; dimensions are points. The default staff is 18 points high.
pub struct Options<'a> {
    /// Available system width.
    pub line_width: f64,
    /// Height of a five-line staff.
    pub staff_size: f64,
    /// Last-line and single-line spacing policy.
    pub justify: Justify,
    /// Optical gap adjustment between opposite stem directions, in staff spaces.
    pub optical_adjustment: f64,
    /// Finite work and size bounds.
    pub limits: Limits,
    /// Optional caller-owned buffer-growth ledger.
    pub reserve: Option<&'a mut Reserve<'a>>,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Self {
            line_width: 564.0,
            staff_size: 18.0,
            justify: Justify::Auto,
            optical_adjustment: 0.4,
            limits: Limits::default(),
            reserve: None,
        }
    }
}

impl fmt::Debug for Options<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Options")
            .field("line_width", &self.line_width)
            .field("staff_size", &self.staff_size)
            .field("justify", &self.justify)
            .field("optical_adjustment", &self.optical_adjustment)
            .field("limits", &self.limits)
            .field("reserve", &self.reserve.is_some())
            .finish()
    }
}

pub(crate) fn failure(message: &str) -> Error {
    Error::new(ErrorKind::EngraveLimit, message)
        .hint("Split the score into smaller examples or increase its available width.")
}

pub(crate) fn charge(left: &mut usize, count: usize) -> Result<(), Error> {
    *left =
        left.checked_sub(count).ok_or_else(|| failure("The engraving work limit was reached."))?;
    Ok(())
}

pub(crate) fn push<T>(
    items: &mut Vec<T>,
    value: T,
    reserve: &mut Option<&mut Reserve<'_>>,
) -> Result<(), Error> {
    if items.len() == items.capacity() {
        let count = items.capacity().max(16);
        let bytes = count
            .checked_mul(std::mem::size_of::<T>())
            .ok_or_else(|| failure("The engraving buffer size overflowed."))?;
        if reserve.as_mut().is_some_and(|r| !r(bytes)) {
            return Err(failure("The build's memory ledger refused an engraving buffer."));
        }
        items
            .try_reserve_exact(count)
            .map_err(|_| failure("Cannot allocate an engraving buffer."))?;
    }
    items.push(value);
    Ok(())
}

pub(crate) fn font_error(message: impl Into<String>) -> Error {
    Error::new(ErrorKind::Font, message)
        .at(Span::default())
        .hint("Provide a valid text face through pdf_font_paths or use embedded fonts.")
}
