//! The line-by-line parser that `GoogleDocstring` and `NumpyDocstring` share: a queue
//! of docstring lines consumed from the front, section headers recognised as each
//! style recognises them, and the formatting helpers the sections use.

use super::patterns::{
    google_typed_arg, is_bullet, is_directive, is_enumerated, is_google_section,
    is_numpy_underline, split_xref_or_code,
};
use super::sections::Section;
use super::types::convert_type_spec;
use super::{DocstringStyle, NapoleonContext, NapoleonNote, Shared};

/// How deeply field descriptions may hold converted docstrings of their own; deeper
/// descriptions are kept as written.
const MAX_DEPTH: usize = 32;

/// One field of a section: a name, a type and the description's lines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Field {
    pub(crate) name: String,
    pub(crate) ty: String,
    pub(crate) desc: Vec<String>,
}

/// One docstring being converted.
pub(crate) struct Parser<'a> {
    pub(crate) style: DocstringStyle,
    pub(crate) shared: &'a Shared<'a>,
    /// `_what`: the object's kind; `"object"` for a field's description.
    pub(crate) what: &'a str,
    pub(crate) name: &'a str,
    pub(crate) no_index: bool,
    /// The object's annotations, when the object is known (`_obj`).
    pub(crate) annotations: Option<&'a [(String, String)]>,
    lines: Vec<String>,
    pos: usize,
    parsed: Vec<String>,
    /// For each parsed line, the input line its chunk started at.
    origins: Vec<usize>,
    pub(crate) in_section: bool,
    pub(crate) section_indent: usize,
    /// The section being read names parameters or attributes, so a field line must
    /// start with a name.
    pub(crate) strict_names: bool,
    depth: usize,
    pub(crate) notes: Vec<NapoleonNote>,
}

/// The number of characters before the first one that is not whitespace.
pub(crate) fn indent_of(line: &str) -> usize {
    line.chars().take_while(|c| c.is_whitespace()).count()
}

/// `line[n:]`, counting characters.
pub(crate) fn chars_from(line: &str, n: usize) -> &str {
    line.char_indices().nth(n).map_or("", |(i, _)| &line[i..])
}

/// `_is_indented`: the first `indent` characters are whitespace, and there are more.
pub(crate) fn is_indented(line: &str, indent: usize) -> bool {
    for (i, c) in line.chars().enumerate() {
        if i >= indent {
            return true;
        }
        if !c.is_whitespace() {
            return false;
        }
    }
    false
}

/// `_get_min_indent`.
fn min_indent(lines: &[String]) -> usize {
    lines.iter().filter(|l| !l.is_empty()).map(|l| indent_of(l)).min().unwrap_or(0)
}

/// `_get_initial_indent`.
pub(crate) fn initial_indent(lines: &[String]) -> usize {
    lines.iter().find(|l| !l.is_empty()).map_or(0, |l| indent_of(l))
}

/// `_dedent`.
pub(crate) fn dedent(lines: Vec<String>) -> Vec<String> {
    let n = min_indent(&lines);
    if n == 0 {
        return lines;
    }
    lines.iter().map(|l| chars_from(l, n).to_owned()).collect()
}

/// `_indent`: every line, empty ones too.
pub(crate) fn indent(lines: &[String], n: usize) -> Vec<String> {
    let pad = " ".repeat(n);
    lines.iter().map(|l| format!("{pad}{l}")).collect()
}

/// `_strip_empty`: without the empty lines at either end.
pub(crate) fn strip_empty(lines: Vec<String>) -> Vec<String> {
    let Some(start) = lines.iter().position(|l| !l.is_empty()) else { return Vec::new() };
    let end = lines.iter().rposition(|l| !l.is_empty()).map_or(start, |e| e + 1);
    if start == 0 && end == lines.len() {
        return lines;
    }
    lines[start..end].to_vec()
}

/// `any(lines)`: some line is not empty.
pub(crate) fn any(lines: &[String]) -> bool {
    lines.iter().any(|l| !l.is_empty())
}

/// `_partition_field_on_colon`: the text before the first colon outside roles and
/// literals, the colon, and the text after it, both stripped.
pub(crate) fn partition_on_colon(line: &str) -> (String, &'static str, String) {
    let (mut before, mut after) = (String::new(), String::new());
    let mut colon = "";
    for (i, part) in split_xref_or_code(line).into_iter().enumerate() {
        if !colon.is_empty() {
            after.push_str(part);
            continue;
        }
        match super::patterns::single_colon(part).filter(|_| i % 2 == 0) {
            Some(at) => {
                colon = ":";
                before.push_str(&part[..at]);
                after.push_str(&part[at + 1..]);
            }
            None => before.push_str(part),
        }
    }
    (before.trim().to_owned(), colon, after.trim().to_owned())
}

/// `_is_list`.
pub(crate) fn is_list(lines: &[String]) -> bool {
    let Some(first) = lines.first() else { return false };
    if is_bullet(first) || is_enumerated(first) {
        return true;
    }
    if lines.len() < 2 || first.ends_with("::") {
        return false;
    }
    let indent = indent_of(first);
    let next = lines[1..].iter().find(|l| !l.is_empty()).map_or(indent, |l| indent_of(l));
    next > indent
}

impl<'a> Parser<'a> {
    /// A parser of `lines` (each stripped at its end) for the object `ctx` describes.
    pub(crate) fn new(
        style: DocstringStyle,
        shared: &'a Shared<'a>,
        ctx: &NapoleonContext<'a>,
        lines: &[String],
    ) -> Parser<'a> {
        Parser {
            style,
            shared,
            what: ctx.what,
            name: ctx.name,
            no_index: ctx.no_index,
            annotations: ctx.annotations,
            lines: lines.iter().map(|l| l.trim_end().to_owned()).collect(),
            pos: 0,
            parsed: Vec::new(),
            origins: Vec::new(),
            in_section: false,
            section_indent: 0,
            strict_names: false,
            depth: 0,
            notes: Vec::new(),
        }
    }

    /// `self.__class__(lines, self._config).lines()`: a description converted on its
    /// own, as a docstring of no particular object.
    pub(crate) fn nested(&self, lines: Vec<String>) -> Vec<String> {
        if self.depth >= MAX_DEPTH {
            return lines;
        }
        let ctx = NapoleonContext { what: "object", ..NapoleonContext::default() };
        let mut p = Parser::new(self.style, self.shared, &ctx, &lines);
        p.depth = self.depth + 1;
        p.parse();
        p.parsed
    }

    /// Records a problem at input line `line`; descriptions converted on their own
    /// report nothing, since their lines are not the docstring's.
    pub(crate) fn note(&mut self, code: &'static str, line: usize, text: String) {
        if self.depth == 0 {
            self.notes.push(NapoleonNote { code, line, text });
        }
    }

    /// The parsed lines and, for each, the input line it came from.
    pub(crate) fn finish(self) -> (Vec<String>, Vec<usize>, Vec<NapoleonNote>) {
        (self.parsed, self.origins, self.notes)
    }

    /// The input line the parser is at.
    pub(crate) fn position(&self) -> usize {
        self.pos
    }

    /// `_lines.get(n)`: the line `n` ahead, or `None` past the end.
    pub(crate) fn peek(&self, n: usize) -> Option<&str> {
        self.lines.get(self.pos + n).map(String::as_str)
    }

    /// `_lines.next()`: the next line; `""` past the end.
    pub(crate) fn next_line(&mut self) -> String {
        let line = self.lines.get(self.pos).cloned().unwrap_or_default();
        self.pos = (self.pos + 1).min(self.lines.len());
        line
    }

    /// `not self._lines`.
    pub(crate) fn at_end(&self) -> bool {
        self.pos >= self.lines.len()
    }

    /// The section a (lower-case) header names.
    pub(crate) fn section(&self, name: &str) -> Option<Section> {
        self.shared.sections.iter().find(|(n, _)| n == name).map(|(_, s)| *s)
    }

    /// `_get_current_indent`: the indent of the first non-empty line from `peek`.
    pub(crate) fn current_indent(&self, peek: usize) -> usize {
        let rest = self.lines.get(self.pos + peek..).unwrap_or_default();
        rest.iter().find(|l| !l.is_empty()).map_or(0, |l| indent_of(l))
    }

    /// `_is_section_header`, in this parser's style.
    pub(crate) fn is_section_header(&self) -> bool {
        let Some(line) = self.peek(0) else { return false };
        let lower = line.to_lowercase();
        let known = match self.style {
            DocstringStyle::Google => {
                is_google_section(&lower)
                    && self.section(lower.trim_matches(':')).is_some()
                    && self.current_indent(1) > indent_of(&lower)
            }
            DocstringStyle::Numpy => {
                self.section(&lower).is_some() && self.peek(1).is_some_and(is_numpy_underline)
            }
        };
        known
            || (self.style == DocstringStyle::Numpy
                && is_directive(&lower)
                && lower.starts_with(".. index::"))
    }

    /// `_is_section_break`.
    pub(crate) fn is_section_break(&self) -> bool {
        let Some(line) = self.peek(0) else { return true };
        if self.is_section_header() {
            return true;
        }
        if self.style == DocstringStyle::Numpy && line.is_empty() && self.peek(1) == Some("") {
            return true;
        }
        self.in_section && !line.is_empty() && !is_indented(line, self.section_indent)
    }

    /// `_consume_indented_block`.
    pub(crate) fn consume_indented_block(&mut self, indent: usize) -> Vec<String> {
        let mut lines = Vec::new();
        while !self.is_section_break() {
            match self.peek(0) {
                Some(l) if l.is_empty() || is_indented(l, indent) => {}
                _ => break,
            }
            lines.push(self.next_line());
        }
        lines
    }

    /// `_consume_contiguous`.
    fn consume_contiguous(&mut self) -> Vec<String> {
        let mut lines = Vec::new();
        while self.peek(0).is_some_and(|l| !l.is_empty()) && !self.is_section_header() {
            lines.push(self.next_line());
        }
        lines
    }

    /// `_consume_empty`.
    pub(crate) fn consume_empty(&mut self) -> Vec<String> {
        let mut lines = Vec::new();
        while self.peek(0) == Some("") {
            lines.push(self.next_line());
        }
        lines
    }

    /// `_consume_to_end`.
    pub(crate) fn consume_to_end(&mut self) -> Vec<String> {
        let rest = self.lines[self.pos..].to_vec();
        self.pos = self.lines.len();
        rest
    }

    /// `_consume_to_next_section`.
    pub(crate) fn consume_to_next_section(&mut self) -> Vec<String> {
        self.consume_empty();
        let mut lines = Vec::new();
        while !self.is_section_break() {
            lines.push(self.next_line());
        }
        lines.extend(self.consume_empty());
        lines
    }

    /// `_escape_args_and_kwargs`: `*args` and `**kwargs` escaped; NumPy escapes each of
    /// `a, *b`.
    pub(crate) fn escape_args(&self, name: &str) -> String {
        if self.style == DocstringStyle::Numpy && name.contains(", ") {
            let parts: Vec<String> = name.split(", ").map(|n| self.escape_one(n)).collect();
            return parts.join(", ");
        }
        self.escape_one(name)
    }

    fn escape_one(&self, name: &str) -> String {
        let name = match name.strip_suffix('_') {
            Some(stem) if self.shared.strip_backslash => format!("{stem}\\_"),
            _ => name.to_owned(),
        };
        if let Some(rest) = name.strip_prefix("**") {
            return format!("\\*\\*{rest}");
        }
        match name.strip_prefix('*') {
            Some(rest) => format!("\\*{rest}"),
            None => name,
        }
    }

    /// `_lookup_annotation`: the annotation of an attribute of the documented module or
    /// class, when `napoleon_attr_annotations` asks for it.
    pub(crate) fn lookup_annotation(&self, name: &str) -> String {
        let wanted = self.shared.settings.attr_annotations
            && matches!(self.what, "module" | "class" | "exception");
        let found = self
            .annotations
            .filter(|_| wanted)
            .and_then(|a| a.iter().find(|(n, _)| n == name).map(|(_, t)| t.clone()));
        found.unwrap_or_default()
    }

    /// A type converted by `napoleon_preprocess_types`, its problems noted at `line`.
    pub(crate) fn preprocess_type(&mut self, ty: &str, line: usize) -> String {
        let mut problems = Vec::new();
        let out = convert_type_spec(ty, &self.shared.settings.type_aliases, &mut problems);
        for p in problems {
            let text = format!("The type specification has an {}: {}.", p.what, p.token);
            self.note("napoleon.type", line, text);
        }
        out
    }

    /// `GoogleDocstring._consume_field`.
    fn consume_google_field(&mut self, parse_type: bool, prefer_type: bool) -> Field {
        let at = self.pos;
        let line = self.next_line();
        let (before, colon, after) = partition_on_colon(&line);
        let (mut name, mut ty) = (before.clone(), String::new());
        if parse_type && let Some((n, t, end)) = google_typed_arg(&before) {
            name = n.trim().to_owned();
            ty = t.to_owned();
            if colon.is_empty() && !prefer_type && !before[end..].trim().is_empty() {
                self.note_field(at, &line);
            }
        } else if parse_type && colon.is_empty() && !prefer_type && has_spaced_name(&before) {
            self.note_field(at, &line);
        }
        let mut name = self.escape_args(&name);
        if prefer_type && ty.is_empty() {
            std::mem::swap(&mut ty, &mut name);
        }
        if !ty.is_empty() && self.shared.settings.preprocess_types {
            ty = self.preprocess_type(&ty, at);
        }
        let mut desc = vec![after];
        let block = self.consume_indented_block(indent_of(&line) + 1);
        desc.extend(dedent(block));
        Field { name, ty, desc: self.nested(desc) }
    }

    /// Notes a field line that names no parameter napoleon can read.
    pub(crate) fn note_field(&mut self, at: usize, line: &str) {
        if !self.strict_names {
            return;
        }
        let text = format!(
            "The field `{}` has no name, type and description napoleon can tell apart.",
            line.trim()
        );
        self.note("napoleon.field", at, text);
    }

    /// `_consume_field`, in this parser's style.
    pub(crate) fn consume_field(&mut self, parse_type: bool, prefer_type: bool) -> Field {
        match self.style {
            DocstringStyle::Google => self.consume_google_field(parse_type, prefer_type),
            DocstringStyle::Numpy => self.consume_numpy_field(parse_type, prefer_type),
        }
    }

    /// `_consume_fields`; with `multiple`, `x, y: int` gives a field per name.
    pub(crate) fn consume_fields(
        &mut self,
        parse_type: bool,
        prefer_type: bool,
        multiple: bool,
    ) -> Vec<Field> {
        self.consume_empty();
        let mut fields = Vec::new();
        while !self.is_section_break() {
            let start = self.pos;
            let f = self.consume_field(parse_type, prefer_type);
            if multiple && !f.name.is_empty() {
                for name in f.name.split(',') {
                    fields.push(Field { name: name.trim().to_owned(), ..f.clone() });
                }
            } else if !f.name.is_empty() || !f.ty.is_empty() || !f.desc.is_empty() {
                fields.push(f);
            }
            if self.pos == start {
                break;
            }
        }
        fields
    }

    /// `_consume_inline_attribute`: the type and description of an attribute's
    /// docstring, `type: description`.
    fn consume_inline_attribute(&mut self) -> (String, Vec<String>) {
        let line = self.next_line();
        let (mut ty, colon, mut desc) = partition_on_colon(&line);
        if colon.is_empty() || desc.is_empty() {
            std::mem::swap(&mut ty, &mut desc);
            desc.push_str(colon);
        }
        let mut lines = vec![desc];
        let rest = self.consume_to_end();
        lines.extend(dedent(rest));
        (ty, self.nested(lines))
    }

    /// `_parse_attribute_docstring`.
    fn parse_attribute_docstring(&mut self) -> Vec<String> {
        if self.at_end() {
            return Vec::new();
        }
        let (ty, desc) = self.consume_inline_attribute();
        let mut lines = format_field("", "", desc);
        if !ty.is_empty() {
            lines.push(String::new());
            lines.push(format!(":type: {ty}"));
        }
        lines
    }

    /// `_consume_section_header`: the header line, as the section table names it.
    fn consume_section_header(&mut self) -> String {
        let section = self.next_line();
        match self.style {
            DocstringStyle::Google => {
                let stripped = section.trim_matches(':');
                if self.section(&stripped.to_lowercase()).is_some() {
                    return stripped.to_owned();
                }
                section
            }
            DocstringStyle::Numpy => {
                if !is_directive(&section) {
                    self.next_line();
                }
                section
            }
        }
    }

    /// Adds `lines`, which came from input line `start` on.
    fn extend(&mut self, lines: Vec<String>, start: usize) {
        self.origins.extend(std::iter::repeat_n(start, lines.len()));
        self.parsed.extend(lines);
    }

    /// `_parse`.
    pub(crate) fn parse(&mut self) {
        let empty = self.consume_empty();
        self.origins.extend(0..empty.len());
        self.parsed = empty;
        if !self.name.is_empty() && matches!(self.what, "attribute" | "data" | "property") {
            let start = self.pos;
            let lines = self.parse_attribute_docstring();
            self.extend(lines, start);
            return;
        }
        while !self.at_end() {
            let start = self.pos;
            let lines = if self.is_section_header() {
                self.parse_section()
            } else {
                let lines = if self.parsed.is_empty() {
                    let mut lines = self.consume_contiguous();
                    lines.extend(self.consume_empty());
                    lines
                } else {
                    self.consume_to_next_section()
                };
                self.note_unindented_sections(start);
                lines
            };
            if self.pos == start {
                // Nothing was consumed; keep the line as written rather than loop.
                let line = self.next_line();
                self.extend(vec![line], start);
                continue;
            }
            self.extend(lines, start);
        }
    }

    /// One section: its header consumed, its body parsed.
    fn parse_section(&mut self) -> Vec<String> {
        let section = self.consume_section_header();
        self.in_section = true;
        self.section_indent = self.current_indent(0);
        let lines = if is_directive(&section) {
            let mut lines = vec![section];
            lines.extend(self.consume_to_next_section());
            lines
        } else {
            match self.section(&section.to_lowercase()) {
                Some(kind) => self.parse_section_body(kind, &section),
                None => vec![section],
            }
        };
        self.in_section = false;
        self.section_indent = 0;
        lines
    }

    /// Notes the Google headers of known sections among the text lines from `start` to
    /// here whose bodies are not indented below them: napoleon reads them as text.
    fn note_unindented_sections(&mut self, start: usize) {
        if self.style != DocstringStyle::Google || self.depth > 0 {
            return;
        }
        for at in start..self.pos {
            let line = &self.lines[at];
            let lower = line.to_lowercase();
            if !is_google_section(&lower) || self.section(lower.trim_matches(':')).is_none() {
                continue;
            }
            let Some(next) = self.lines[at + 1..].iter().find(|l| !l.is_empty()) else {
                continue;
            };
            if indent_of(next) > indent_of(line) {
                continue;
            }
            let text = format!(
                "The section `{}` has a body that is not indented below its header, so it \
                 is read as text.",
                line.trim()
            );
            self.note("napoleon.section", at, text);
            return;
        }
    }
}

/// Whether a field's name part holds more than one word in some comma-separated part:
/// `x description` rather than `x` or `x, y`.
pub(crate) fn has_spaced_name(name: &str) -> bool {
    name.split(',').any(|part| part.split_whitespace().nth(1).is_some())
}

/// `_fix_field_desc`.
pub(crate) fn fix_field_desc(desc: Vec<String>) -> Vec<String> {
    if is_list(&desc) {
        let mut out = vec![String::new()];
        out.extend(desc);
        return out;
    }
    let Some(first) = desc.first().filter(|f| f.ends_with("::")) else { return desc };
    let block = &desc[1..];
    if initial_indent(block) > indent_of(first) {
        let mut out = vec![String::new()];
        out.extend(desc);
        return out;
    }
    let mut out = vec![String::new(), first.clone()];
    out.extend(indent(block, 4));
    out
}

/// `_format_admonition`.
pub(crate) fn format_admonition(admonition: &str, lines: Vec<String>) -> Vec<String> {
    let lines = strip_empty(lines);
    match lines.len() {
        0 => vec![format!(".. {admonition}::"), String::new()],
        1 => vec![format!(".. {admonition}:: {}", lines[0].trim()), String::new()],
        _ => {
            let mut out = vec![format!(".. {admonition}::"), String::new()];
            out.extend(indent(&dedent(lines), 3));
            out.push(String::new());
            out
        }
    }
}

/// `_format_block`: `prefix` before the first line, its width of spaces before the
/// others.
pub(crate) fn format_block(prefix: &str, lines: &[String]) -> Vec<String> {
    let padding = " ".repeat(prefix.chars().count());
    format_block_padded(prefix, lines, &padding)
}

/// `_format_block` with an explicit padding.
pub(crate) fn format_block_padded(prefix: &str, lines: &[String], padding: &str) -> Vec<String> {
    if lines.is_empty() {
        return vec![prefix.to_owned()];
    }
    let mut out = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            out.push(format!("{prefix}{line}").trim_end().to_owned());
        } else if line.is_empty() {
            out.push(String::new());
        } else {
            out.push(format!("{padding}{line}"));
        }
    }
    out
}

/// `_format_field`: `**name** (*type*) -- description`.
pub(crate) fn format_field(name: &str, ty: &str, desc: Vec<String>) -> Vec<String> {
    let desc = strip_empty(desc);
    let has_desc = any(&desc);
    let separator = if has_desc { " -- " } else { "" };
    let field = match (name.is_empty(), ty.is_empty()) {
        (false, false) if ty.contains('`') => format!("**{name}** ({ty}){separator}"),
        (false, false) => format!("**{name}** (*{ty}*){separator}"),
        (false, true) => format!("**{name}**{separator}"),
        (true, false) if ty.contains('`') => format!("{ty}{separator}"),
        (true, false) => format!("*{ty}*{separator}"),
        (true, true) => String::new(),
    };
    if !has_desc {
        return vec![field];
    }
    let mut desc = fix_field_desc(desc);
    if desc[0].is_empty() {
        desc.insert(0, field);
    } else {
        desc[0] = format!("{field}{}", desc[0]);
    }
    desc
}
