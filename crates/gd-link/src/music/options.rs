use gd_engrave::Justify;
use gd_env::OutputFormat;
use guidedoc::{Document, NodeId};

use crate::page::Page;

pub(super) fn dimensions<'a>(
    page: &Page<'_>,
    doc: Document<'_>,
    id: NodeId,
) -> Result<gd_engrave::Options<'a>, String> {
    let config = &page.config().lilypond;
    let base = if page.format == OutputFormat::Pdf {
        config.pdf_width.or(page.music_width).unwrap_or(419.53)
    } else {
        config.html_width.unwrap_or(564.0)
    };
    let width = doc
        .custom_attribute(id, "width")
        .map(|value| length(value, base))
        .transpose()?
        .unwrap_or(base);
    let staff_size = doc
        .custom_attribute(id, "staff-size")
        .map(|value| length(value, 18.0))
        .transpose()?
        .or(config.staff_size)
        .unwrap_or(18.0);
    if !(10.0..=40.0).contains(&staff_size) {
        return Err("Music staff-size must be between 10pt and 40pt.".into());
    }
    let justify = match doc.custom_attribute(id, "justify").unwrap_or("auto") {
        "auto" => Justify::Auto,
        "all" => Justify::All,
        "none" => Justify::None,
        value => return Err(format!("Unknown music justification `{value}`.")),
    };
    Ok(gd_engrave::Options { line_width: width, staff_size, justify, ..Default::default() })
}

fn length(value: &str, base: f64) -> Result<f64, String> {
    let value = value.trim();
    // The directive parser accepts every CSS length, but engraving needs one fixed measure
    // for both builders, and a PDF page has no viewport.
    if ["vmin", "vmax", "vw", "vh"].iter().any(|unit| value.ends_with(unit)) {
        return Err(format!(
            "Viewport unit in `{value}`: \
             music needs a percentage of the measure or a fixed length."
        ));
    }
    let suffixes = [
        ("rem", 12.0),
        ("em", 12.0),
        ("ex", 6.0),
        ("ch", 6.0),
        ("pt", 1.0),
        ("pc", 12.0),
        ("mm", 72.0 / 25.4),
        ("cm", 720.0 / 25.4),
        ("Q", 72.0 / 101.6),
        ("in", 72.0),
        ("px", 0.75),
        ("%", base / 100.0),
    ];
    let (number, scale) = suffixes
        .iter()
        .find_map(|(suffix, scale)| value.strip_suffix(suffix).map(|number| (number, *scale)))
        .unwrap_or((value, 1.0));
    let points = number.trim().parse::<f64>().unwrap_or(f64::NAN) * scale;
    if !points.is_finite() || points <= 0.0 || points > 100_000.0 {
        return Err(format!("Invalid music dimension `{value}`."));
    }
    Ok(points)
}

#[cfg(test)]
mod tests {
    #[test]
    fn widths_accept_print_and_relative_units_but_not_viewport_units() {
        assert_eq!(super::length("50%", 400.0), Ok(200.0));
        assert_eq!(super::length("2in", 400.0), Ok(144.0));
        for unit in ["80vw", "50vh", "10vmin", "10vmax"] {
            let error = super::length(unit, 400.0).unwrap_err();
            assert!(error.contains("Viewport unit"), "{error}");
        }
    }
}
