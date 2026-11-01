use crate::{Error, ErrorKind, TextStyle};

/// Horizontal alignment within a vertical markup column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColumnAlign {
    /// Left-aligned lines (`\column`, `\left-column`).
    #[default]
    Left,
    /// Centered lines (`\center-column`).
    Center,
    /// Right-aligned lines (`\right-column`).
    Right,
}

/// A decorative frame or rule drawn around a markup expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Enclosure {
    /// Rectangular frame (`\box`, `\rounded-box`).
    Box,
    /// Circular or oval frame (`\circle`).
    Circle,
    /// Baseline rule (`\underline`).
    Underline,
}

/// Semantic role for a structured markup annotation on an event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MarkupRole {
    /// Performance text script (`^\markup`, `_\markup`, `\markup`).
    #[default]
    Script,
    /// Tempo indication (`\tempo \markup ...`).
    Tempo,
    /// Rehearsal or structural mark (`\mark \markup ...`).
    Rehearsal,
}

/// How a markup command changes the enclosing style; changes nest, as in LilyPond.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleChange {
    /// `\bold`: adds weight.
    Bold,
    /// `\italic`: adds slant.
    Italic,
    /// `\upright`: removes slant and keeps weight.
    Upright,
    /// `\medium`: removes weight and keeps slant.
    Medium,
    /// `\normal-text`: removes both.
    Normal,
}

impl StyleChange {
    /// The style that results from applying this change inside `outer`.
    pub fn apply(self, outer: TextStyle) -> TextStyle {
        let bold = matches!(outer, TextStyle::Bold | TextStyle::BoldItalic);
        let italic = matches!(outer, TextStyle::Italic | TextStyle::BoldItalic);
        let (bold, italic) = match self {
            Self::Bold => (true, italic),
            Self::Italic => (bold, true),
            Self::Upright => (bold, false),
            Self::Medium => (false, italic),
            Self::Normal => (false, false),
        };
        match (bold, italic) {
            (false, false) => TextStyle::Regular,
            (true, false) => TextStyle::Bold,
            (false, true) => TextStyle::Italic,
            (true, true) => TextStyle::BoldItalic,
        }
    }
}

/// Rich formatted text and musical symbols independent of LilyPond syntax.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Markup {
    /// Shaped text string.
    Text(String),
    /// Horizontal sequence separated by word spaces (`\line`, `{ ... }`).
    Line(Vec<Self>),
    /// Horizontal sequence with no separating spaces (`\concat`).
    Concat(Vec<Self>),
    /// Vertical stack of lines (`\column`, `\center-column`, `\right-column`).
    Column(ColumnAlign, Vec<Self>),
    /// A change to the enclosing text style (`\bold`, `\italic`, `\upright`, ...).
    Style(StyleChange, Box<Self>),
    /// Relative size percentage and vertical baseline shift in hundredths of an em.
    Scaled {
        /// Font size percentage relative to the enclosing markup (`100` is unchanged).
        scale_percent: u16,
        /// Upward baseline shift in hundredths of the enclosing font size.
        shift_percent: i16,
        /// Scaled child expression.
        body: Box<Self>,
    },
    /// Frame or underline around an expression.
    Framed(Enclosure, Box<Self>),
    /// Named SMuFL glyph from the embedded music font.
    MusicGlyph(String),
    /// Inline dynamic string rendered with SMuFL dynamic glyphs.
    Dynamic(String),
    /// Explicit horizontal and vertical offset in hundredths of a staff space.
    Space {
        /// Horizontal advance in hundredths of a staff space.
        horizontal: i16,
        /// Vertical advance in hundredths of a staff space.
        vertical: i16,
    },
}

impl Markup {
    /// Extracts a readable plain-text summary for accessibility descriptions and metadata.
    pub fn plain_text(&self) -> String {
        let mut out = String::new();
        self.write_plain(&mut out);
        out
    }

    /// Returns a conservative heap footprint estimate in bytes.
    pub fn approx_bytes(&self) -> usize {
        match self {
            Self::Text(s) | Self::MusicGlyph(s) | Self::Dynamic(s) => s.len().saturating_add(64),
            Self::Line(items) | Self::Concat(items) | Self::Column(_, items) => {
                items.iter().fold(64usize, |acc, item| acc.saturating_add(item.approx_bytes()))
            }
            Self::Style(_, body) | Self::Framed(_, body) | Self::Scaled { body, .. } => {
                body.approx_bytes().saturating_add(64)
            }
            Self::Space { .. } => 64,
        }
    }

    /// Validates depth, scale percentages and dynamic letters.
    pub fn check(&self, depth: usize) -> Result<(), Error> {
        if depth > 64 {
            return Err(Error::new(ErrorKind::Score, "Markup nesting exceeds depth 64."));
        }
        match self {
            Self::Text(_) | Self::MusicGlyph(_) | Self::Space { .. } => Ok(()),
            Self::Dynamic(text) => {
                if text.is_empty() || !text.chars().all(|c| "pmfrsz".contains(c)) {
                    return Err(Error::new(
                        ErrorKind::Score,
                        "Markup dynamic contains unsupported characters.",
                    ));
                }
                Ok(())
            }
            Self::Line(items) | Self::Concat(items) | Self::Column(_, items) => {
                for item in items {
                    item.check(depth + 1)?;
                }
                Ok(())
            }
            Self::Style(_, body) | Self::Framed(_, body) => body.check(depth + 1),
            Self::Scaled { scale_percent, shift_percent, body } => {
                if !(10..=1000).contains(scale_percent) || !(-1000..=1000).contains(shift_percent)
                {
                    return Err(Error::new(
                        ErrorKind::Score,
                        "Markup scale or baseline shift is out of bounds.",
                    ));
                }
                body.check(depth + 1)
            }
        }
    }

    fn write_plain(&self, out: &mut String) {
        match self {
            Self::Text(text) | Self::Dynamic(text) => out.push_str(text),
            Self::Line(items) | Self::Column(_, items) => {
                for (index, item) in items.iter().enumerate() {
                    if index > 0 && !out.ends_with(' ') {
                        out.push(' ');
                    }
                    item.write_plain(out);
                }
            }
            Self::Concat(items) => {
                for item in items {
                    item.write_plain(out);
                }
            }
            Self::Style(_, body) | Self::Framed(_, body) | Self::Scaled { body, .. } => {
                body.write_plain(out);
            }
            Self::MusicGlyph(name) => out.push_str(match name.as_str() {
                "accidentalFlat" => "b",
                "accidentalSharp" => "#",
                "accidentalNatural" => "♮",
                "accidentalDoubleFlat" => "bb",
                "accidentalDoubleSharp" => "##",
                _ => "",
            }),
            Self::Space { horizontal, .. } => {
                if *horizontal > 0 && !out.ends_with(' ') {
                    out.push(' ');
                }
            }
        }
    }
}
