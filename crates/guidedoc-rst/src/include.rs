//! The include and raw directives, and the resources directives load. The resource
//! provider, include depth, cycles, and byte budgets are the engine's
//! (`ReadContext::include_source`).

use guidedoc::{
    Arg, AttributeKey, Fail, NodeKind, SourceId, SourceSpan, SourceText, SourceUse, Text, arg_span,
};

use crate::directives::{DirOption, Directive};
use crate::lines::{Line, join_lines, split_lines, strip_bom, trim_blank_end};
use crate::messages::*;
use crate::names::parse_int;
use crate::parser::Parser;
use crate::specs::equal_fold;

/// Joins path lines without white space, as the specification treats paths.
pub fn path_of(lines: &[Line<'_>]) -> String {
    join_lines(lines, "").into_owned()
}

/// The byte offset of line `n` (0-based, negative from the end, as a Python slice
/// bound) in `text`.
fn line_offset(text: &str, n: i64) -> usize {
    let mut total = text.bytes().filter(|&c| c == b'\n').count() as i64;
    if !text.is_empty() && !text.ends_with('\n') {
        total += 1;
    }
    let k = if n < 0 { (n + total).max(0) } else { n };
    if k >= total {
        return text.len();
    }
    let mut seen = 0;
    for (i, c) in text.bytes().enumerate() {
        if seen == k {
            return i;
        }
        if c == b'\n' {
            seen += 1;
        }
    }
    text.len()
}

impl<'p, 'a> Parser<'p, 'a> {
    /// Refuses encodings other than UTF-8.
    pub(crate) fn check_encoding(&mut self, d: &Directive<'_>) -> bool {
        let (Some(v), Some(o)) = (d.option_value("encoding"), d.option("encoding")) else {
            return true;
        };
        if ["utf-8", "utf8", "utf-8-sig"].iter().any(|e| equal_fold(&v, e)) {
            return true;
        }
        self.report_at(DIRECTIVE_ENCODING, o.span);
        false
    }

    /// Loads the resource option `o` names, resolved by the host against the current
    /// source; `None` when it was refused (and reported).
    pub(crate) fn resource_text(
        &mut self,
        o: &DirOption<'_>,
    ) -> Option<(SourceId, SourceText<'a>)> {
        let path = path_of(&o.lines);
        let id = self.ctx.include_source(self.st.src, &path, o.span, SourceUse::Literal)?;
        Some((id, self.ctx.source_of(id)))
    }

    /// Applies `:start-line:`, `:end-line:`, `:start-after:`, and `:end-before:`.
    fn include_range(
        &mut self,
        d: &Directive<'_>,
        text: &str,
    ) -> Result<Option<(usize, usize)>, Fail> {
        let mut a = d.option_value("start-line").map_or(0, |v| line_offset(text, parse_int(&v)));
        let mut z =
            d.option_value("end-line").map_or(text.len(), |v| line_offset(text, parse_int(&v)));
        z = z.max(a);
        let mut work = (z - a) as u64 + 1;
        for name in ["start-after", "end-before"] {
            if let Some(v) = d.option_text(name) {
                work = work.saturating_mul(v.len() as u64 + 1);
            }
        }
        self.ctx.b.spend(work / 16 + 1)?;
        if let Some(v) = d.option_text("start-after") {
            let Some(k) = text.get(a..z).and_then(|s| s.find(&*v)) else {
                return Ok(self.marker_missing(d, "start-after"));
            };
            a += k + v.len();
        }
        if let Some(v) = d.option_text("end-before") {
            let Some(k) = text.get(a..z).and_then(|s| s.find(&*v)) else {
                return Ok(self.marker_missing(d, "end-before"));
            };
            z = a + k;
        }
        Ok(Some((a, z)))
    }

    fn marker_missing<T>(&mut self, d: &Directive<'_>, name: &str) -> Option<T> {
        if let Some(o) = d.option(name) {
            self.report_at(INCLUDE_TEXT_NOT_FOUND, o.span);
        }
        None
    }

    /// Parses another source in place, or includes it as a literal or code block.
    pub(crate) fn include(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        if let Some(o) = d.option("parser") {
            self.report_at(DIRECTIVE_PARSER, o.span);
            return Ok(());
        }
        if !self.check_encoding(d) {
            return Ok(());
        }
        let path = path_of(&d.arg);
        let Some(id) = self.ctx.include_source(self.st.src, &path, d.span, SourceUse::Parsed)
        else {
            return Ok(());
        };
        let text = self.ctx.source_of(id);
        let tab = d.option_value("tab-width").map(|v| parse_int(&v)).filter(|&n| n > 0);
        let tab = tab.map_or(self.st.tab, |n| n.min(1 << 16) as usize);
        let Some((mut a, z)) = self.include_range(d, &text)? else {
            return Ok(());
        };
        if a == 0 && text.starts_with('\u{feff}') {
            a = 3.min(z);
        }
        let mut buf = String::new();
        let lines = split_lines(text.get(a..z).unwrap_or(""), a as u32, tab, &mut buf);
        let code = d.option_value("code");
        if code.is_none() && !d.has_option("literal") {
            return self.in_source(id, tab, |p| p.parse_body(&lines, d.sections, None));
        }
        self.in_source(id, tab, |p| {
            let body = trim_blank_end(&lines);
            let block = p.open(NodeKind::CodeBlock, SourceSpan::new(id, a as u32, z as u32))?;
            if let Some(code) = code.filter(|c| !c.is_empty()) {
                p.ctx.b.add_attribute_string(block, AttributeKey::Language, &code)?;
            }
            p.add_common(block, d, "class")?;
            p.number_lines(block, d);
            let text = p.code_text(body)?;
            p.ctx.b.set_text(block, text);
            p.close(block)
        })
    }

    /// The raw content of a raw directive, from its content or `:file:`; `None` when it
    /// was refused (and reported).
    fn raw_text(&mut self, d: &Directive<'_>) -> Result<Option<Text>, Fail> {
        if let Some(o) = d.option("url") {
            self.report_at(DIRECTIVE_URL, o.span);
            return Ok(None);
        }
        if !self.check_encoding(d) {
            return Ok(None);
        }
        let at = arg_span(d.name_span);
        let Some(o) = d.option("file") else {
            if d.content.is_empty() {
                self.report(DIRECTIVE_CONTENT_MISSING, d.span, at, Arg::None);
                return Ok(None);
            }
            return self.code_text(&d.content).map(Some);
        };
        if !d.content.is_empty() {
            self.report(DIRECTIVE_FILE_AND_CONTENT, d.span, at, Arg::None);
            return Ok(None);
        }
        let Some((_, text)) = self.resource_text(o) else {
            return Ok(None);
        };
        let (body, base) = strip_bom(&text);
        let mut buf = String::new();
        let lines = split_lines(body, base, self.st.tab, &mut buf);
        self.code_text(trim_blank_end(&lines)).map(Some)
    }

    /// Emits a RawBlock, or a RawInline in a substitution definition. The renderer
    /// applies the raw content policy.
    pub(crate) fn raw_directive(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        let Some(text) = self.raw_text(d)? else {
            return Ok(());
        };
        let kind = if d.subst { NodeKind::RawInline } else { NodeKind::RawBlock };
        let id = self.open(kind, d.span)?;
        // The format in lower case with single spaces.
        let format: Vec<String> =
            d.argument().split_ascii_whitespace().map(str::to_ascii_lowercase).collect();
        self.ctx.b.add_attribute_string(id, AttributeKey::Format, &format.join(" "))?;
        self.add_common(id, d, "class")?;
        self.ctx.b.set_text(id, text);
        self.close(id)
    }
}
