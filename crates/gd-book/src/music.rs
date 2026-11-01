//! The selected template declares the text measure used by native music layout.

pub(crate) fn text_width(template: &str, paper: &str) -> f64 {
    let margin = template
        .lines()
        .take(64)
        .find_map(|line| {
            let value = line.trim().strip_prefix("// guidedog: margin-x =")?.trim();
            let mm = value.strip_suffix("mm")?.trim().parse::<f64>().ok()?;
            (mm.is_finite() && (0.0..100.0).contains(&mm)).then_some(mm)
        })
        .unwrap_or(25.0);
    let width = if matches!(paper, "letter" | "us-letter") { 215.9 } else { 210.0 };
    (width - margin * 2.0) * 72.0 / 25.4
}

#[cfg(test)]
mod tests {
    #[test]
    fn selected_template_and_paper_determine_music_measure() {
        let width = super::text_width("#import x\n// guidedog: margin-x = 31mm\n", "a4");
        assert!((width - 148.0 * 72.0 / 25.4).abs() < 1e-9);
        assert!((super::text_width("", "letter") - 165.9 * 72.0 / 25.4).abs() < 1e-9);
        assert_eq!(
            super::text_width("// guidedog: margin-x = NaNmm", "a4"),
            super::text_width("", "a4")
        );
    }
}
