//! Tables with spans, figures, and images with their sizes.

use guidedoc::{
    Align, AttributeKey, CellInfo, LinkKind, MediaKind, NodeId, NodeKind, arg_span, arg_text,
    media_kind,
};

use crate::inlines::{has_prefix_fold, url_unsafe};
use crate::messages::{DIMENSION_IGNORED, IMAGE_REMOTE};
use crate::settings::parse_count;
use crate::state::State;

/// The words of a widths list such as "1 2 1" or "30,70".
fn words(s: &str) -> impl Iterator<Item = &str> {
    s.split([' ', ',', '\t']).filter(|w| !w.is_empty())
}

/// Counts positive integer widths, or `None` for anything else.
fn count_widths(widths: &str) -> Option<usize> {
    let mut n = 0;
    for word in words(widths) {
        parse_count(word).filter(|&v| v > 0)?;
        n += 1;
    }
    Some(n)
}

impl State<'_, '_> {
    /// Writes `#table` with a `table.header` for head rows and `table.cell` for spanning
    /// or aligned cells. A table with a title becomes a figure with a caption.
    pub(crate) fn render_table(&mut self, id: NodeId) {
        let doc = self.doc;
        let title = doc.first_child(id).filter(|&t| doc.kind(t) == NodeKind::Title);
        if let Some(title) = title {
            self.emit("#figure(");
            if !self.custom(id, "number-label").is_empty() {
                self.emit("numbering: none, ");
            }
            self.emit("caption: [");
            self.emit_caption_number(id);
            self.wrap_inlines("", title, "], kind: table, table(");
        } else {
            self.emit("#table(");
        }
        self.emit("columns: ");
        self.emit_columns(id);
        self.emit(",\n");
        for part in doc.children(id) {
            let kind = doc.kind(part);
            if kind == NodeKind::TableHead {
                self.emit("table.header(\n");
                self.emit_rows(part);
                // The rule under the head, which repeats with it on each page.
                self.emit("table.hline(),\n),\n");
            } else if kind == NodeKind::TableBody {
                self.emit_rows(part);
            }
        }
        self.emit(if title.is_some() { "))\n" } else { ")\n" });
    }

    /// Writes relative widths when the Widths attribute gives one per column, and the
    /// column count otherwise.
    fn emit_columns(&mut self, id: NodeId) {
        let mut columns = usize::try_from(self.doc.node(id).value).unwrap_or(0);
        if columns == 0 {
            columns = self.count_columns(id);
        }
        if let Some(widths) = self.doc.attribute(id, AttributeKey::Widths)
            && columns > 0
            && count_widths(widths) == Some(columns)
        {
            self.byte(b'(');
            for word in words(widths) {
                self.emit(word);
                self.emit("fr, ");
            }
            self.byte(b')');
            return;
        }
        self.int(columns.max(1) as i64);
    }

    /// Sums the column spans of the first row.
    fn count_columns(&self, table: NodeId) -> usize {
        let doc = self.doc;
        let first_row = (table.0..doc.subtree_end(table).0)
            .map(NodeId)
            .find(|&i| doc.kind(i) == NodeKind::Row);
        match first_row {
            Some(row) => doc.children(row).map(|c| usize::from(doc.cell_info(c).col_span)).sum(),
            None => 1,
        }
    }

    fn emit_rows(&mut self, part: NodeId) {
        let doc = self.doc;
        for row in doc.children(part) {
            self.map_node(row);
            for cell in doc.children(row) {
                let info = doc.cell_info(cell);
                if info.col_span > 1 || info.row_span > 1 || info.align != Align::None {
                    self.emit_cell_arguments(info);
                }
                self.emit_block_argument(cell);
                self.emit(",\n");
            }
        }
    }

    fn emit_cell_arguments(&mut self, info: CellInfo) {
        self.emit("table.cell(");
        let mut comma = "";
        if info.col_span > 1 {
            self.emit("colspan: ");
            self.int(i64::from(info.col_span));
            comma = ", ";
        }
        if info.row_span > 1 {
            self.emit(comma);
            self.emit("rowspan: ");
            self.int(i64::from(info.row_span));
            comma = ", ";
        }
        let align = match info.align {
            Align::None => "",
            Align::Left => "left",
            Align::Center => "center",
            Align::Right => "right",
        };
        if !align.is_empty() {
            self.emit(comma);
            self.emit("align: ");
            self.emit(align);
        }
        self.byte(b')');
    }

    /// Writes `#figure` around the image, with the caption; a legend follows.
    pub(crate) fn render_figure(&mut self, id: NodeId) {
        let doc = self.doc;
        self.emit("#figure(");
        if !self.custom(id, "number-label").is_empty() {
            self.emit("numbering: none, ");
        }
        let mut body = false;
        for part in doc.children(id) {
            let kind = doc.kind(part);
            if kind == NodeKind::Image {
                self.emit_image(part, false);
            } else if matches!(kind, NodeKind::Link | NodeKind::Reference) {
                self.emit_linked_image(part);
            } else if kind == NodeKind::Caption {
                if !body {
                    self.emit("[]");
                }
                self.emit(", caption: [");
                self.emit_caption_number(id);
                self.wrap_inlines("", part, "]");
            } else {
                continue;
            }
            body = true;
        }
        if !body {
            self.emit("[]");
        }
        self.emit(")\n");
        for legend in doc.children(id).filter(|&p| doc.kind(p) == NodeKind::Legend) {
            self.map_node(legend);
            self.emit("\n#gd-legend[\n");
            self.render_blocks(legend);
            self.emit("]\n");
        }
    }

    /// Writes a figure image wrapped in its `:target:` link.
    fn emit_linked_image(&mut self, link: NodeId) {
        let doc = self.doc;
        let Some(image) = doc.first_child(link).filter(|&i| doc.kind(i) == NodeKind::Image) else {
            self.emit("[]");
            return;
        };
        let info = doc.link_info(link);
        let kind = info.map(|i| i.kind);
        let destination = info.map_or("", |i| doc.text(i.destination));
        let external = matches!(kind, Some(LinkKind::External | LinkKind::Email))
            && self.url_allowed(link, destination);
        let labelled = match info {
            Some(i) if i.kind == LinkKind::Internal && !external => {
                self.labelled_target(i.resolved)
            }
            _ => None,
        };
        let wrapped = external || labelled.is_some();
        if external {
            self.emit("link(");
            self.emit_url(destination, kind == Some(LinkKind::Email));
            self.emit(", ");
        } else if let Some(node) = labelled {
            let name = self.anchors.anchor_of(node).unwrap_or("");
            let out = &mut self.ctx.out;
            out.emit("gd-link(");
            crate::escape::write_string_literal(out, name);
            out.emit(", ");
        }
        self.emit_image(image, false);
        if wrapped {
            self.byte(b')');
        }
    }

    /// Writes an image expression in code mode. Paths stay as written, so they resolve
    /// relative to the generated file. Typst cannot fetch remote images; those become
    /// links showing the alternative text, and audio and video name the recording.
    pub(crate) fn emit_image(&mut self, id: NodeId, block: bool) {
        let doc = self.doc;
        let path = doc.link_info(id).map_or("", |i| doc.text(i.destination));
        if is_remote(path) {
            self.emit_remote_image(id, path);
            return;
        }
        let kind = media_kind(path);
        if kind != MediaKind::Image {
            // A book cannot play media; it names the recording, which pages play.
            self.emit("gd-media(");
            self.emit_string(if kind == MediaKind::Video { "video" } else { "audio" });
            self.emit(", ");
            self.emit_string(path);
            self.emit(", [");
            let file = path.rsplit('/').next().unwrap_or(path);
            self.emit_alt_text(id, file);
            self.emit("])");
            return;
        }
        let align = doc
            .attribute(id, AttributeKey::Align)
            .filter(|a| block && matches!(*a, "left" | "center" | "right"));
        if let Some(align) = align {
            self.emit("gd-align(");
            self.emit(align);
            self.emit(", ");
        }
        let scaled = self.emit_scale_open(id);
        self.emit("image(");
        self.emit_string(path);
        self.emit_alt(id);
        self.emit_dimension(id, AttributeKey::Width, ", width: ");
        self.emit_dimension(id, AttributeKey::Height, ", height: ");
        self.byte(b')');
        if scaled {
            self.byte(b')');
        }
        if align.is_some() {
            self.byte(b')');
        }
    }

    fn emit_remote_image(&mut self, id: NodeId, path: &str) {
        let span = self.doc.node(id).span;
        self.fail(IMAGE_REMOTE, span, arg_span(span));
        let plain = url_unsafe(path) || has_prefix_fold(path, "data:") || self.quiet;
        if plain {
            self.emit("[");
        } else {
            self.emit("link(");
            self.emit_url(path, false);
            self.emit(", [");
        }
        self.emit_alt_text(id, path);
        self.emit(if plain { "]" } else { "])" });
    }

    /// Writes the `alt:` argument from the Alt attribute or the description inlines.
    fn emit_alt(&mut self, id: NodeId) {
        if let Some(alt) = self.doc.attribute(id, AttributeKey::Alt) {
            self.emit(", alt: ");
            self.emit_string(alt);
        } else if self.doc.has_children(id) {
            self.emit(", alt: ");
            self.emit_plain_string(id);
        }
    }

    fn emit_alt_text(&mut self, id: NodeId, fallback: &str) {
        if let Some(alt) = self.doc.attribute(id, AttributeKey::Alt).filter(|a| !a.is_empty()) {
            self.emit_text(alt);
        } else if self.doc.has_children(id) {
            self.render_inlines(id);
        } else {
            self.emit_text(fallback);
        }
    }

    /// Opens `gd-scale(...)` for an RST `:scale:` percentage.
    fn emit_scale_open(&mut self, id: NodeId) -> bool {
        let Some(text) = self.doc.attribute(id, AttributeKey::Scale) else {
            return false;
        };
        let percent = parse_count(text.strip_suffix('%').unwrap_or(text)).filter(|&p| p > 0);
        let Some(percent) = percent else {
            let span = self.doc.node(id).span;
            self.fail(DIMENSION_IGNORED, span, arg_text(text));
            return false;
        };
        self.emit("gd-scale(");
        self.int(percent);
        self.emit("%, ");
        true
    }

    /// Converts an RST or HTML length. Pixels become points at 96 px per inch, ex
    /// becomes half an em, and a bare number counts as pixels.
    fn emit_dimension(&mut self, id: NodeId, key: AttributeKey, prefix: &str) {
        let Some(text) = self.doc.attribute(id, key).filter(|t| !t.is_empty()) else {
            return;
        };
        let Some((milli, unit)) = parse_length(text) else {
            let span = self.doc.node(id).span;
            self.fail(DIMENSION_IGNORED, span, arg_text(text));
            return;
        };
        let (milli, unit) = match unit {
            "" | "px" => (milli * 3 / 4, "pt"),
            "pc" => (milli * 12, "pt"),
            "ex" => (milli / 2, "em"),
            other => (milli, other),
        };
        self.emit(prefix);
        self.int(milli / 1000);
        let fraction = milli % 1000;
        if fraction != 0 {
            let digits = fraction_digits(fraction);
            self.byte(b'.');
            self.ctx.out.emit_bytes(&digits[..fraction_length(&digits)]);
        }
        self.emit(unit);
    }
}

/// Returns thousandths as three decimal digits.
fn fraction_digits(fraction: i64) -> [u8; 3] {
    let digit = |v: i64| b'0' + (v % 10) as u8;
    [digit(fraction / 100), digit(fraction / 10), digit(fraction)]
}

/// Returns how many of `digits` remain without trailing zeros.
fn fraction_length(digits: &[u8; 3]) -> usize {
    digits.iter().rposition(|&d| d != b'0').map_or(0, |i| i + 1)
}

/// Tells whether `path` names a remote resource: a scheme followed by "//", a
/// protocol-relative "//" path, or a data URL.
fn is_remote(path: &str) -> bool {
    if has_prefix_fold(path, "data:") || has_prefix_fold(path, "//") {
        return true;
    }
    let b = path.as_bytes();
    for (i, &c) in b.iter().enumerate() {
        if c == b':' {
            return i > 1 && i + 2 < b.len() && b[i + 1] == b'/' && b[i + 2] == b'/';
        }
        if !c.is_ascii_alphanumeric() && !matches!(c, b'+' | b'-' | b'.') {
            return false;
        }
    }
    false
}

/// The units a length may have.
const UNITS: [&str; 10] = ["", "px", "pt", "pc", "mm", "cm", "in", "em", "ex", "%"];

/// Reads "12", "1.5cm", or "50 %" into thousandths and a known unit.
pub(crate) fn parse_length(text: &str) -> Option<(i64, &str)> {
    let b = text.as_bytes();
    let (mut milli, mut digits, mut point, mut scale) = (0i64, 0, false, 1000i64);
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_digit() {
            digits += 1;
            if digits > 9 {
                return None;
            }
            let d = i64::from(c - b'0');
            if !point {
                milli = milli * 10 + d * 1000;
            } else if scale > 1 {
                scale /= 10;
                milli += d * scale;
            }
        } else if c == b'.' && !point {
            point = true;
        } else {
            break;
        }
        i += 1;
    }
    if digits == 0 {
        return None;
    }
    let unit = text[i..].trim_start_matches(' ');
    UNITS.contains(&unit).then_some((milli, unit))
}
