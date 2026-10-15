//! Doc comments (CommonMark, as rustdoc reads them) to reStructuredText: the block
//! structure here, the inline text in [`crate::inline`].
//!
//! The common subset is translated: paragraphs, headings (as rubrics, since sections
//! cannot sit inside a directive), fenced and indented code (as `code-block`, Rust by
//! default with rustdoc's hidden `# ` lines removed), bullet and ordered lists, block
//! quotes, pipe tables (as `list-table`), HTML blocks (as `raw:: html`), thematic breaks
//! (dropped), link reference definitions (used for links, then dropped), footnotes (as
//! auto-numbered footnotes at the end), images (as `image` directives or substitutions)
//! and hard line breaks (a paragraph with one becomes a line block).
//!
//! Lists and quotes nest by recursion bounded by [`MAX_NESTING`]; deeper content is
//! written as plain paragraphs.

use crate::inline::{Image, Inline, LinkResolver, escape_plain, strip_markup};
use std::collections::HashMap;

/// How deeply lists and block quotes may nest before their content is flattened.
pub(crate) const MAX_NESTING: usize = 12;

/// The indentation reST directive content takes.
const BODY: &str = "   ";

/// Writes `markdown` as reST lines indented by `indent` into `out`. Links are resolved
/// through `links`. Footnote and image names start with `names`, which must differ
/// between the doc comments of one page. Writes nothing for blank docs.
pub(crate) fn write_rst(
    markdown: &str,
    indent: &str,
    names: &str,
    links: &mut dyn LinkResolver,
    out: &mut String,
) {
    let definitions = take_definitions(markdown);
    let footnote_names: HashMap<String, String> = definitions
        .footnotes
        .iter()
        .enumerate()
        .map(|(i, (label, _))| (label.to_lowercase(), format!("{names}-f{}", i + 1)))
        .collect();
    let inline = Inline {
        links,
        refs: &definitions.refs,
        footnotes: &footnote_names,
        names,
        images: vec![],
    };
    let mut converter = Converter { inline };
    let mut body = String::new();
    converter.blocks(&definitions.lines, 0, &mut body);
    for (i, (_, lines)) in definitions.footnotes.iter().enumerate() {
        converter.footnote(&format!("{names}-f{}", i + 1), lines, &mut body);
    }
    for image in &converter.inline.images {
        write_substitution(image, &mut body);
    }
    for line in body.lines() {
        if !line.is_empty() {
            out.push_str(indent);
            out.push_str(line);
        }
        out.push('\n');
    }
}

/// The first paragraph of `markdown` as one line of plain text, for synopses.
pub(crate) fn summary(markdown: &str) -> String {
    let paragraph: Vec<&str> = markdown
        .lines()
        .skip_while(|l| l.trim().is_empty() || l.trim_start().starts_with('#'))
        .take_while(|l| !l.trim().is_empty())
        .collect();
    let joined = paragraph.join(" ");
    let plain = strip_markup(&joined).split_whitespace().collect::<Vec<_>>().join(" ");
    first_sentence(&plain).to_string()
}

/// The first sentence: up to a `.` followed by a space and a capital letter.
fn first_sentence(text: &str) -> &str {
    let bytes = text.as_bytes();
    for (i, _) in text.match_indices(". ") {
        if bytes.get(i + 2).is_some_and(u8::is_ascii_uppercase) {
            return &text[..=i];
        }
    }
    text
}

/// A doc comment with its definitions taken out.
struct Definitions<'t> {
    /// The lines left to convert.
    lines: Vec<&'t str>,
    /// Link reference definitions by lower-cased label.
    refs: HashMap<String, String>,
    /// Footnote definitions in source order: the label and the content's lines.
    footnotes: Vec<(String, Vec<&'t str>)>,
}

/// Splits link reference definitions (`[label]: destination`) and footnote definitions
/// (`[^label]: text`, with indented continuation lines) off the text, outside code
/// fences.
fn take_definitions(markdown: &str) -> Definitions<'_> {
    let all: Vec<&str> = markdown.lines().collect();
    let mut taken = Definitions { lines: Vec::new(), refs: HashMap::new(), footnotes: Vec::new() };
    let mut fence: Option<String> = None;
    let mut i = 0;
    while i < all.len() {
        let line = all[i];
        let trimmed = line.trim_start();
        i += 1;
        if let Some(open) = &fence {
            if trimmed.starts_with(open.as_str()) {
                fence = None;
            }
            taken.lines.push(line);
            continue;
        }
        if let Some((marker, _)) = fence_marker(line) {
            fence = Some(marker);
            taken.lines.push(line);
            continue;
        }
        if indent_of(line) >= 4 {
            taken.lines.push(line);
        } else if let Some((label, first)) = footnote_definition(trimmed) {
            let (content, next) = footnote_content(&all, i, first);
            taken.footnotes.push((label, content));
            i = next;
        } else if let Some((label, destination)) = reference_definition(trimmed) {
            taken.refs.insert(label.to_lowercase(), destination);
        } else {
            taken.lines.push(line);
        }
    }
    taken
}

/// `[^label]: text` gives the label and the text.
fn footnote_definition(line: &str) -> Option<(String, &str)> {
    let rest = line.strip_prefix("[^")?;
    let (label, text) = rest.split_once("]:")?;
    let label = label.trim();
    let valid = !label.is_empty() && !label.contains(char::is_whitespace);
    valid.then(|| (label.to_string(), text.trim_start()))
}

/// The lines of a footnote whose first line is `first`: following lines indented by
/// four (blank lines between them included) or lazily continuing its paragraph. Returns
/// them dedented and the index after them.
fn footnote_content<'t>(lines: &[&'t str], start: usize, first: &'t str) -> (Vec<&'t str>, usize) {
    let mut content = vec![first];
    let mut i = start;
    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            let next = lines[i + 1..].iter().find(|l| !l.trim().is_empty());
            if next.is_none_or(|l| indent_of(l) < 4) {
                break;
            }
            content.push("");
        } else if indent_of(line) >= 4 {
            content.push(dedent(line, 4));
        } else if content.last().is_some_and(|l| !l.trim().is_empty())
            && !interrupts_paragraph(line)
            && footnote_definition(line.trim_start()).is_none()
            && reference_definition(line.trim_start()).is_none()
        {
            content.push(line.trim_start()); // A lazy continuation line.
        } else {
            break;
        }
        i += 1;
    }
    (content, i)
}

/// `[label]: destination` gives the label and the destination.
fn reference_definition(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix('[')?;
    let (label, after) = rest.split_once("]:")?;
    if label.trim().is_empty() || label.starts_with('^') {
        return None;
    }
    let destination = after.split_whitespace().next()?;
    Some((label.trim().to_string(), destination.trim_matches(['<', '>']).to_string()))
}

/// The number of columns of leading whitespace (a tab counts four).
fn indent_of(line: &str) -> usize {
    line.chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

/// Removes up to `n` columns of leading whitespace.
fn dedent(line: &str, n: usize) -> &str {
    let mut removed = 0;
    for (i, c) in line.char_indices() {
        if removed >= n || !(c == ' ' || c == '\t') {
            return &line[i..];
        }
        removed += if c == '\t' { 4 } else { 1 };
    }
    ""
}

/// The marker of a code fence opening line (three or more backticks or tildes) and
/// its info string.
fn fence_marker(line: &str) -> Option<(String, String)> {
    if indent_of(line) > 3 {
        return None;
    }
    let trimmed = line.trim_start();
    let c = trimmed.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let n = trimmed.chars().take_while(|x| *x == c).count();
    if n < 3 {
        return None;
    }
    let info = trimmed[n..].trim();
    if c == '`' && info.contains('`') {
        return None;
    }
    Some((c.to_string().repeat(n), info.to_string()))
}

/// What a list item marker is: a bullet character, or an ordered number and its
/// delimiter. `width` is the columns up to the item's content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ListMarker {
    bullet: Option<char>,
    number: Option<u32>,
    delimiter: char,
    /// The bytes of the marker itself (`-`, `12.`).
    marker_len: usize,
    width: usize,
}

impl ListMarker {
    fn same_list(&self, other: &ListMarker) -> bool {
        self.bullet == other.bullet
            && self.number.is_some() == other.number.is_some()
            && self.delimiter == other.delimiter
    }
}

fn list_marker(line: &str) -> Option<ListMarker> {
    let indent = indent_of(line);
    if indent > 3 {
        return None;
    }
    let rest = line.trim_start();
    let (bullet, number, delimiter, marker_len) = match rest.chars().next()? {
        c @ ('-' | '+' | '*') => (Some(c), None, c, 1),
        c if c.is_ascii_digit() => {
            let digits = rest.chars().take_while(char::is_ascii_digit).count();
            let delimiter = rest[digits..].chars().next().filter(|d| *d == '.' || *d == ')')?;
            if digits > 9 {
                return None;
            }
            (None, rest[..digits].parse().ok(), delimiter, digits + 1)
        }
        _ => return None,
    };
    let after = &rest[marker_len..];
    if !after.is_empty() && !after.starts_with([' ', '\t']) {
        return None;
    }
    let spaces = indent_of(after);
    let gap = if after.trim().is_empty() || spaces > 4 { 1 } else { spaces };
    Some(ListMarker { bullet, number, delimiter, marker_len, width: indent + marker_len + gap })
}

fn is_thematic_break(line: &str) -> bool {
    if indent_of(line) > 3 {
        return false;
    }
    let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    let Some(first) = compact.chars().next() else { return false };
    matches!(first, '-' | '*' | '_') && compact.len() >= 3 && compact.chars().all(|c| c == first)
}

fn atx_heading(line: &str) -> Option<&str> {
    if indent_of(line) > 3 {
        return None;
    }
    let trimmed = line.trim_start();
    let level = trimmed.chars().take_while(|c| *c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = &trimmed[level..];
    if !rest.is_empty() && !rest.starts_with([' ', '\t']) {
        return None;
    }
    Some(rest.trim().trim_end_matches('#').trim_end())
}

fn is_setext_underline(line: &str) -> bool {
    let trimmed = line.trim();
    indent_of(line) < 4
        && !trimmed.is_empty()
        && (trimmed.chars().all(|c| c == '=') || trimmed.chars().all(|c| c == '-'))
}

fn is_html_start(line: &str) -> bool {
    let trimmed = line.trim_start();
    let mut chars = trimmed.chars();
    indent_of(line) < 4
        && chars.next() == Some('<')
        && chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '/' || c == '!')
        && trimmed.contains('>')
}

fn is_table_delimiter(line: &str) -> bool {
    let trimmed = line.trim();
    let has_columns = trimmed.contains('|') || trimmed.starts_with([':', '-']);
    trimmed.contains('-')
        && has_columns
        && trimmed.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t'))
}

/// Whether `line` starts a block that ends a paragraph before it.
fn interrupts_paragraph(line: &str) -> bool {
    let ordered_from_one =
        list_marker(line).is_some_and(|m| m.bullet.is_some() || m.number == Some(1));
    let nonempty_item = ordered_from_one && !line.trim_start()[1..].trim().is_empty();
    fence_marker(line).is_some()
        || atx_heading(line).is_some()
        || line.trim_start().starts_with('>') && indent_of(line) < 4
        || is_thematic_break(line)
        || nonempty_item
        || is_html_start(line)
}

/// The kind of language a fenced block's info string gives, rustdoc's way: no info or
/// only rustdoc's test attributes (`ignore`, `no_run`, ...) mean Rust.
pub(crate) fn code_language(info: &str) -> String {
    const RUSTDOC_ATTRIBUTES: [&str; 8] = [
        "rust",
        "ignore",
        "should_panic",
        "no_run",
        "compile_fail",
        "test_harness",
        "standalone_crate",
        "allow_fail",
    ];
    for token in info.split(|c: char| c == ',' || c.is_whitespace()) {
        let token = token.trim().trim_start_matches('{').trim_end_matches('}');
        let is_rustdoc = RUSTDOC_ATTRIBUTES.contains(&token)
            || token.starts_with("edition")
            || token.starts_with("ignore-")
            || (token.starts_with('E') && token[1..].chars().all(|c| c.is_ascii_digit()));
        if token.is_empty() || is_rustdoc {
            continue;
        }
        return match token {
            "plain" | "txt" => "text".to_string(),
            other => other.trim_start_matches('.').to_string(),
        };
    }
    "rust".to_string()
}

/// Rustdoc hides lines of Rust examples that start with `# ` (or are just `#`); `##`
/// writes a literal `#`.
fn shown_rust_line(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if trimmed == "#" || trimmed.starts_with("# ") {
        return None;
    }
    if trimmed.starts_with("##") {
        let indent = line.len() - trimmed.len();
        return Some(&line[indent + 1..]);
    }
    Some(line)
}

struct Converter<'r> {
    inline: Inline<'r>,
}

impl Converter<'_> {
    /// Converts a sequence of block lines, appending reST to `out` with blank lines
    /// between blocks.
    fn blocks(&mut self, lines: &[&str], depth: usize, out: &mut String) {
        let mut i = 0;
        while i < lines.len() {
            if lines[i].trim().is_empty() {
                i += 1;
                continue;
            }
            if !out.is_empty() && !out.ends_with("\n\n") {
                out.push('\n');
            }
            i = self.block(lines, i, depth, out);
        }
    }

    /// Converts the block starting at line `i`; returns the line after it.
    fn block(&mut self, lines: &[&str], i: usize, depth: usize, out: &mut String) -> usize {
        let line = lines[i];
        if let Some((marker, info)) = fence_marker(line) {
            return fenced_code(lines, i, &marker, &info, out);
        }
        if indent_of(line) >= 4 {
            return indented_code(lines, i, out);
        }
        if let Some(title) = atx_heading(line) {
            self.rubric(title, out);
            return i + 1;
        }
        if is_thematic_break(line) {
            return i + 1;
        }
        if depth < MAX_NESTING {
            if line.trim_start().starts_with('>') {
                return self.quote(lines, i, depth, out);
            }
            if let Some(marker) = list_marker(line) {
                return self.list(lines, i, marker, depth, out);
            }
        }
        if lines.get(i + 1).is_some_and(|next| is_table_delimiter(next)) && line.contains('|') {
            return self.table(lines, i, out);
        }
        if is_html_start(line) {
            return html(lines, i, out);
        }
        self.paragraph(lines, i, out)
    }

    fn rubric(&mut self, title: &str, out: &mut String) {
        let title = self.inline.convert(title);
        out.push_str(".. rubric:: ");
        out.push_str(&title);
        out.push('\n');
    }

    fn paragraph(&mut self, lines: &[&str], start: usize, out: &mut String) -> usize {
        let mut end = start + 1;
        while end < lines.len() {
            let line = lines[end];
            if is_setext_underline(line) {
                let text: Vec<&str> = lines[start..end].iter().map(|l| l.trim()).collect();
                self.rubric(&text.join(" "), out);
                return end + 1;
            }
            if line.trim().is_empty() || interrupts_paragraph(line) {
                break;
            }
            end += 1;
        }
        let segments = hard_break_segments(&lines[start..end]);
        if segments.len() > 1 {
            self.line_block(&segments, out);
            return end;
        }
        let converted = self.inline.convert(&segments.concat());
        if !self.standalone_image(&converted, out) {
            out.push_str(&safe_paragraph(&converted));
            out.push('\n');
        }
        end
    }

    /// A paragraph with hard line breaks: a line block, one line per segment.
    fn line_block(&mut self, segments: &[String], out: &mut String) {
        for segment in segments {
            let converted = self.inline.convert(segment);
            out.push('|');
            if !converted.is_empty() {
                out.push(' ');
                out.push_str(&converted);
            }
            out.push('\n');
        }
    }

    /// A paragraph that is one image (`|d3-img1|`) is written as an `image` directive
    /// instead of a substitution. Returns whether it was.
    fn standalone_image(&mut self, converted: &str, out: &mut String) -> bool {
        let images = &mut self.inline.images;
        let is_last_image = images.last().is_some_and(|i| converted == format!("|{}|", i.name));
        let Some(image) = images.pop_if(|_| is_last_image) else {
            return false;
        };
        out.push_str(".. image:: ");
        out.push_str(&image.src);
        out.push('\n');
        write_image_options(&image, out);
        true
    }

    /// Writes a footnote definition: `.. [#name]` and its content as blocks.
    fn footnote(&mut self, name: &str, lines: &[&str], out: &mut String) {
        let mut body = String::new();
        self.blocks(lines, 0, &mut body);
        if !out.is_empty() && !out.ends_with("\n\n") {
            out.push('\n');
        }
        out.push_str(&format!(".. [#{name}]"));
        let mut body_lines = body.lines();
        if let Some(first) = body_lines.next().filter(|l| !l.is_empty()) {
            out.push(' ');
            out.push_str(first);
        }
        out.push('\n');
        for line in body_lines {
            if !line.is_empty() {
                out.push_str(BODY);
                out.push_str(line);
            }
            out.push('\n');
        }
    }

    fn quote(&mut self, lines: &[&str], start: usize, depth: usize, out: &mut String) -> usize {
        let mut inner = Vec::new();
        let mut end = start;
        while end < lines.len() {
            let trimmed = lines[end].trim_start();
            if let Some(rest) = trimmed.strip_prefix('>') {
                inner.push(rest.strip_prefix(' ').unwrap_or(rest));
            } else if !trimmed.is_empty() && !interrupts_paragraph(lines[end]) && end > start {
                inner.push(trimmed); // A lazy continuation line of the quoted paragraph.
            } else {
                break;
            }
            end += 1;
        }
        let mut body = String::new();
        self.blocks(&inner, depth + 1, &mut body);
        // An empty comment ends whatever came before, so the indented quote is not read
        // as more content of a preceding directive or list.
        out.push_str("..\n\n");
        push_indented(&body, BODY, out);
        end
    }

    fn list(
        &mut self,
        lines: &[&str],
        start: usize,
        first: ListMarker,
        depth: usize,
        out: &mut String,
    ) -> usize {
        let mut i = start;
        let mut number = first.number.unwrap_or(1);
        while i < lines.len() {
            let Some(marker) = list_marker(lines[i]).filter(|m| m.same_list(&first)) else {
                break;
            };
            let (item, next) = list_item(lines, i, marker);
            let mut body = String::new();
            self.blocks(&item, depth + 1, &mut body);
            let label = match first.bullet {
                Some(_) => "-".to_string(),
                None => format!("{number}."),
            };
            write_item(&label, &body, out);
            number += 1;
            i = next;
            // Items of one list may be separated by blank lines.
            let mut look = i;
            while look < lines.len() && lines[look].trim().is_empty() {
                look += 1;
            }
            if look < lines.len() && list_marker(lines[look]).is_some_and(|m| m.same_list(&first))
            {
                i = look;
            }
        }
        i
    }

    fn table(&mut self, lines: &[&str], start: usize, out: &mut String) -> usize {
        let header = table_cells(lines[start]);
        let mut rows = vec![header];
        let mut end = start + 2;
        while end < lines.len() && lines[end].contains('|') && !lines[end].trim().is_empty() {
            rows.push(table_cells(lines[end]));
            end += 1;
        }
        let columns = rows.iter().map(Vec::len).max().unwrap_or(1);
        out.push_str(".. list-table::\n   :header-rows: 1\n\n");
        for row in &rows {
            for column in 0..columns {
                let cell = row.get(column).map_or("", String::as_str);
                let converted = self.inline.convert(cell);
                let bullet = if column == 0 { "   * -" } else { "     -" };
                out.push_str(bullet);
                if !converted.is_empty() {
                    out.push(' ');
                    out.push_str(&safe_paragraph(&converted));
                }
                out.push('\n');
            }
        }
        end
    }
}

/// The text of a paragraph's lines, split at hard line breaks: a line ending in two
/// spaces or a backslash (not the last line, where they mean nothing). Lines are
/// trimmed and joined with spaces.
fn hard_break_segments(lines: &[&str]) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    for (i, line) in lines.iter().enumerate() {
        let last = i + 1 == lines.len();
        let trimmed = line.trim();
        let backslashes = trimmed.chars().rev().take_while(|&c| c == '\\').count();
        let (text, breaks) = if last {
            (trimmed, false)
        } else if line.ends_with("  ") {
            (trimmed, true)
        } else if backslashes % 2 == 1 {
            (trimmed[..trimmed.len() - 1].trim_end(), true)
        } else {
            (trimmed, false)
        };
        if !current.is_empty() && !text.is_empty() {
            current.push(' ');
        }
        current.push_str(text);
        if breaks {
            segments.push(std::mem::take(&mut current));
        }
    }
    segments.push(current);
    segments
}

/// Writes the definition of an image used in running text.
fn write_substitution(image: &Image, out: &mut String) {
    if !out.is_empty() && !out.ends_with("\n\n") {
        out.push('\n');
    }
    out.push_str(&format!(".. |{}| image:: {}\n", image.name, image.src));
    write_image_options(image, out);
}

/// The `:alt:` and `:target:` options of an image directive.
fn write_image_options(image: &Image, out: &mut String) {
    let alt = image.alt.split_whitespace().collect::<Vec<_>>().join(" ");
    if !alt.is_empty() {
        out.push_str(&format!("{BODY}:alt: {alt}\n"));
    }
    if let Some(target) = &image.target {
        out.push_str(&format!("{BODY}:target: {target}\n"));
    }
}

/// The lines of the list item starting at `start` (with its marker removed and its
/// content dedented) and the index after it.
fn list_item<'t>(lines: &[&'t str], start: usize, marker: ListMarker) -> (Vec<&'t str>, usize) {
    let after_marker = &lines[start].trim_start()[marker.marker_len..];
    let gap = marker.width - marker.marker_len - indent_of(lines[start]);
    let mut item = vec![dedent(after_marker, gap)];
    let mut i = start + 1;
    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            let next = lines[i + 1..].iter().find(|l| !l.trim().is_empty());
            if next.is_none_or(|l| indent_of(l) < marker.width) {
                break;
            }
            item.push("");
        } else if indent_of(line) >= marker.width {
            item.push(dedent(line, marker.width));
        } else if !interrupts_paragraph(line)
            && list_marker(line).is_none()
            && item.last().is_some_and(|l| !l.trim().is_empty())
        {
            item.push(line.trim_start()); // A lazy continuation line.
        } else {
            break;
        }
        i += 1;
    }
    (item, i)
}

/// Writes one list item: its label, then its body with following lines indented to
/// the content column.
fn write_item(label: &str, body: &str, out: &mut String) {
    let pad = " ".repeat(label.len() + 1);
    let mut lines = body.lines();
    out.push_str(label);
    match lines.next() {
        Some(first) if !first.is_empty() => {
            out.push(' ');
            out.push_str(first);
        }
        _ => {}
    }
    out.push('\n');
    for line in lines {
        if !line.is_empty() {
            out.push_str(&pad);
            out.push_str(line);
        }
        out.push('\n');
    }
}

fn push_indented(body: &str, indent: &str, out: &mut String) {
    for line in body.lines() {
        if !line.is_empty() {
            out.push_str(indent);
            out.push_str(line);
        }
        out.push('\n');
    }
}

/// Keeps a converted paragraph from being read as another reST construct: a first
/// character reST would take for a list, field, comment or quote is preceded by an
/// escaped space, and a final `::` (a literal block marker in reST) is escaped.
fn safe_paragraph(text: &str) -> String {
    let mut text = text.to_string();
    if text.ends_with("::") {
        text.insert(text.len() - 1, '\\');
    }
    let first = text.chars().next();
    let numbered = text.split_once(['.', ')']).is_some_and(|(n, rest)| {
        !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) && rest.starts_with(' ')
    });
    // Markup this crate writes (`*`, a backquote, a role) is safe at the start; literal
    // `*`, backquotes and `|` were escaped already.
    let is_role = text.starts_with(":rust:");
    let risky = !is_role && first.is_some_and(|c| !c.is_alphanumeric() && !"(\"'\\*`".contains(c));
    if risky || numbered {
        text.insert_str(0, "\\ ");
    }
    text
}

fn table_cells(line: &str) -> Vec<String> {
    let trimmed = line.trim().trim_start_matches('|');
    let trimmed = trimmed.strip_suffix('|').unwrap_or(trimmed);
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut chars = trimmed.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cell.push('|');
                chars.next();
            }
            '|' => cells.push(std::mem::take(&mut cell).trim().to_string()),
            _ => cell.push(c),
        }
    }
    cells.push(cell.trim().to_string());
    cells
}

fn fenced_code(lines: &[&str], start: usize, marker: &str, info: &str, out: &mut String) -> usize {
    let indent = indent_of(lines[start]);
    let mut end = start + 1;
    let mut body = Vec::new();
    while end < lines.len() {
        let trimmed = lines[end].trim_start();
        let closes = trimmed.starts_with(marker)
            && trimmed.trim_start_matches(marker.chars().next().unwrap_or('`')).trim().is_empty();
        if closes && indent_of(lines[end]) < 4 {
            end += 1;
            break;
        }
        body.push(dedent(lines[end], indent));
        end += 1;
    }
    write_code(&code_language(info), &body, out);
    end
}

fn indented_code(lines: &[&str], start: usize, out: &mut String) -> usize {
    let mut end = start;
    let mut body = Vec::new();
    while end < lines.len() && (lines[end].trim().is_empty() || indent_of(lines[end]) >= 4) {
        body.push(dedent(lines[end], 4));
        end += 1;
    }
    write_code("rust", &body, out);
    end
}

/// Writes a `code-block` directive; Rust blocks lose rustdoc's hidden lines. Blank
/// edges are trimmed and an empty block is not written.
fn write_code(language: &str, body: &[&str], out: &mut String) {
    let shown: Vec<&str> = if language == "rust" {
        body.iter().filter_map(|l| shown_rust_line(l)).collect()
    } else {
        body.to_vec()
    };
    let first = shown.iter().position(|l| !l.trim().is_empty());
    let last = shown.iter().rposition(|l| !l.trim().is_empty());
    let (Some(first), Some(last)) = (first, last) else { return };
    out.push_str(".. code-block:: ");
    out.push_str(language);
    out.push_str("\n\n");
    for line in &shown[first..=last] {
        let line = line.trim_end();
        if !line.is_empty() {
            out.push_str(BODY);
            out.push_str(line);
        }
        out.push('\n');
    }
}

/// An HTML block runs to the next blank line and is passed through as `raw:: html`.
fn html(lines: &[&str], start: usize, out: &mut String) -> usize {
    let mut end = start;
    out.push_str(".. raw:: html\n\n");
    while end < lines.len() && !lines[end].trim().is_empty() {
        out.push_str(BODY);
        out.push_str(lines[end].trim_end());
        out.push('\n');
        end += 1;
    }
    end
}

/// Plain text written safely as a reST paragraph, for synopses and option values.
pub(crate) fn plain_paragraph(text: &str) -> String {
    safe_paragraph(&escape_plain(text))
}

#[cfg(test)]
mod tests;
