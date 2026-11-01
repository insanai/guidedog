use super::*;

#[test]
fn rasterization_checks_dimensions_pixels_and_reservations() {
    let source = br#"<svg xmlns="http://www.w3.org/2000/svg" width="20pt" height="10pt" viewBox="0 0 20 10"><path d="M1 1L19 9" stroke="black"/></svg>"#;
    let options = RasterOptions { width: 20.0, height: 10.0, ..RasterOptions::default() };
    let result = rasterize_svg(source, &options);
    if EMBEDDED {
        let png = result.expect("embedded PNG export");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().expect("width")), 40);
        assert_eq!(u32::from_be_bytes(png[20..24].try_into().expect("height")), 20);
    } else {
        assert_eq!(result, Err(RasterError::Disabled));
    }
    let refused = |_: usize| false;
    assert_eq!(
        rasterize_svg(source, &RasterOptions { reserve: Some(&refused), ..options }),
        Err(RasterError::Budget)
    );
    for (width, height) in [(0.0, 10.0), (f64::NAN, 10.0), (20.0, -1.0)] {
        assert_eq!(
            rasterize_svg(source, &RasterOptions { width, height, ..RasterOptions::default() }),
            Err(RasterError::Dimensions)
        );
    }
    assert_eq!(
        rasterize_svg(
            source,
            &RasterOptions { width: 3000.0, height: 3000.0, ..RasterOptions::default() }
        ),
        Err(RasterError::Limit)
    );
}

#[cfg(feature = "embedded")]
#[test]
fn embedded_text_faces_include_separate_regular_bold_and_italic_handles() {
    let fonts = load_text_fonts(&Options { embedded_fonts_only: true, ..Options::default() });
    for (bold, italic) in [(false, false), (true, false), (false, true)] {
        let face = fonts
            .faces
            .iter()
            .find(|f| f.family == "Libertinus Serif" && f.bold == bold && f.italic == italic)
            .expect("embedded serif style");
        assert!(!face.data().expect("font bytes").is_empty());
        assert_eq!(face.index, 0);
    }
}
