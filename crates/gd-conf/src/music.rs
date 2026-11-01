//! Music measures are explicit point values; absent overrides follow the builder.

use gd_host::Problem;

use crate::Config;
use crate::check::invalid;

/// Project defaults for native LilyPond engraving; all lengths are points.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LilypondConfig {
    /// `lilypond_staff_size`: None uses an 18-point five-line staff.
    pub staff_size: Option<f64>,
    /// `lilypond_html_width`: None uses the stock theme's 564-point measure.
    pub html_width: Option<f64>,
    /// `lilypond_pdf_width`: None derives the measure from the paper and template.
    pub pdf_width: Option<f64>,
}

pub(crate) fn check(config: &Config, path: &str) -> Result<(), Problem> {
    let staff = config.lilypond.staff_size.unwrap_or(18.0);
    if !staff.is_finite() || !(10.0..=40.0).contains(&staff) {
        return Err(invalid(
            path,
            "lilypond_staff_size must be from 10 to 40 points.".into(),
            "Use lilypond_staff_size = 18 for the default staff.",
        ));
    }
    for (name, width) in [
        ("lilypond_html_width", config.lilypond.html_width),
        ("lilypond_pdf_width", config.lilypond.pdf_width),
    ] {
        if width.is_some_and(|w| !w.is_finite() || w <= 0.0 || w > 100_000.0) {
            return Err(invalid(
                path,
                format!("{name} must be a positive width up to 100000 points."),
                "Give a width in points, or leave the setting out to follow the builder.",
            ));
        }
    }
    Ok(())
}
