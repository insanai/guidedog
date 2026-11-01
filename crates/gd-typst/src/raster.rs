//! SVG rasterization uses the embedded renderer without compiling document code.

use std::fmt;

/// PNG dimensions and memory policy. Width and height are explicit typographic points.
pub struct RasterOptions<'a> {
    /// Positive drawing width in points.
    pub width: f64,
    /// Positive drawing height in points.
    pub height: f64,
    /// Pixels per inch, default 144.
    pub dpi: f64,
    /// Consulted before reserving the SVG, RGBA pixmap and PNG encoding allowance.
    pub reserve: Option<&'a dyn Fn(usize) -> bool>,
}

impl Default for RasterOptions<'_> {
    fn default() -> Self {
        Self { width: 0.0, height: 0.0, dpi: 144.0, reserve: None }
    }
}

impl fmt::Debug for RasterOptions<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RasterOptions")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("dpi", &self.dpi)
            .field("reserve", &self.reserve.is_some())
            .finish()
    }
}

/// Why a standalone SVG could not become a PNG.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RasterError {
    /// The compiler and image renderer are not linked into this build.
    Disabled,
    /// A dimension or resolution is not positive and finite.
    Dimensions,
    /// A side exceeds 16384 pixels, the area exceeds 16 million pixels, or SVG exceeds 64 MiB.
    Limit,
    /// The caller refused the peak rasterization memory allowance.
    Budget,
    /// SVG decoding or PNG encoding failed.
    Data(String),
    /// The renderer failed internally.
    Internal,
}

impl fmt::Display for RasterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => f.write_str("PNG music requires the embedded image renderer."),
            Self::Dimensions => {
                f.write_str("PNG dimensions and resolution must be positive and finite.")
            }
            Self::Limit => f.write_str("The image exceeds the SVG or 16 million pixel limit."),
            Self::Budget => f.write_str("The memory ledger refused SVG rasterization."),
            Self::Data(message) => write!(f, "Cannot rasterize the SVG: {message}"),
            Self::Internal => f.write_str("The embedded image renderer failed internally."),
        }
    }
}

impl std::error::Error for RasterError {}

/// Borrows self-contained SVG and raster settings and returns a transparent PNG.
/// Reads no linked files, runs no document code, and fails before allocating oversized pixmaps.
pub fn rasterize_svg(svg: &[u8], options: &RasterOptions<'_>) -> Result<Vec<u8>, RasterError> {
    if ![options.width, options.height, options.dpi].iter().all(|n| n.is_finite() && *n > 0.0) {
        return Err(RasterError::Dimensions);
    }
    let scale = options.dpi / 72.0;
    if ![options.width as f32, options.height as f32, scale as f32]
        .iter()
        .all(|n| n.is_finite() && *n > 0.0)
    {
        return Err(RasterError::Dimensions);
    }
    let width = (options.width as f32 * scale as f32).round().max(1.0);
    let height = (options.height as f32 * scale as f32).round().max(1.0);
    let pixels = f64::from(width) * f64::from(height);
    if width > 16384.0 || height > 16384.0 || pixels > 16_000_000.0 || svg.len() > 64 * 1024 * 1024
    {
        return Err(RasterError::Limit);
    }
    // Include the decoder's geometry, not just its borrowed source bytes.
    let allowance = svg.len().saturating_mul(16).saturating_add(pixels as usize * 9 + 65536);
    if options.reserve.is_some_and(|reserve| !reserve(allowance)) {
        return Err(RasterError::Budget);
    }
    render(svg, options)
}

#[cfg(feature = "embedded")]
fn render(svg: &[u8], options: &RasterOptions<'_>) -> Result<Vec<u8>, RasterError> {
    use typst::foundations::{Bytes, Content, Smart};
    use typst::layout::{Abs, Frame, FrameItem, FrameKind, Point, Sides, Size};
    use typst::syntax::Span;
    use typst::visualize::{Image, SvgImage};

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let decoded = SvgImage::new(Bytes::new(svg.to_vec()))
            .map_err(|e| RasterError::Data(format!("{e:?}")))?;
        let size = Size::new(Abs::pt(options.width), Abs::pt(options.height));
        let mut frame = Frame::new(size, FrameKind::Hard);
        frame.push(Point::zero(), FrameItem::Image(Image::plain(decoded), size, Span::detached()));
        let page = typst_layout::Page {
            frame,
            bleed: Sides::default(),
            fill: Smart::Custom(None),
            numbering: None,
            supplement: Content::empty(),
            number: 1,
        };
        let options = typst_render::RenderOptions {
            pixel_per_pt: typst::utils::Scalar::new(options.dpi / 72.0),
            ..Default::default()
        };
        let mut png = typst_render::render(&page, &options)
            .encode_png()
            .map_err(|e| RasterError::Data(e.to_string()))?;
        set_resolution(&mut png, options.pixel_per_pt.get() * 72.0)?;
        Ok(png)
    }));
    typst::comemo::evict(0);
    result.map_err(|_| RasterError::Internal)?
}

#[cfg(feature = "embedded")]
fn set_resolution(png: &mut Vec<u8>, dpi: f64) -> Result<(), RasterError> {
    let per_meter = (dpi / 0.0254).round();
    if !(1.0..=f64::from(u32::MAX)).contains(&per_meter) {
        return Err(RasterError::Dimensions);
    }
    if png.len() < 33 || &png[12..16] != b"IHDR" {
        return Err(RasterError::Internal);
    }
    // pHYs preserves typographic size when a PDF embeds the raster edition.
    let mut chunk = [0u8; 21];
    chunk[..4].copy_from_slice(&9u32.to_be_bytes());
    chunk[4..8].copy_from_slice(b"pHYs");
    chunk[8..12].copy_from_slice(&(per_meter as u32).to_be_bytes());
    chunk[12..16].copy_from_slice(&(per_meter as u32).to_be_bytes());
    chunk[16] = 1;
    let mut crc = u32::MAX;
    for byte in &chunk[4..17] {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    chunk[17..].copy_from_slice(&(!crc).to_be_bytes());
    png.try_reserve_exact(chunk.len()).map_err(|_| RasterError::Budget)?;
    let length = png.len();
    png.resize(length + chunk.len(), 0);
    png.copy_within(33..length, 33 + chunk.len());
    png[33..54].copy_from_slice(&chunk);
    Ok(())
}

#[cfg(not(feature = "embedded"))]
fn render(_svg: &[u8], _options: &RasterOptions<'_>) -> Result<Vec<u8>, RasterError> {
    Err(RasterError::Disabled)
}
