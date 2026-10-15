//! Section titles, transitions, and the section hierarchy.

use guidedoc::{Fail, NodeKind, SourceSpan};

use crate::lines::{Line, trim_blank_end};
use crate::messages::*;
use crate::parser::{Construct, MAX_LEVELS, Parser, SectionEntry, Style, detect, is_adornment};
use crate::width::column_width;

impl Parser<'_, '_> {
    /// Handles an unindented text line: a paragraph, a definition list item, or a title
    /// with an underline, as the next line decides.
    pub(crate) fn text_block(
        &mut self,
        lines: &[Line<'_>],
        i: usize,
        sections: bool,
    ) -> Result<usize, Fail> {
        let next = i + 1;
        if next >= lines.len() || lines[next].is_blank() {
            return self.paragraph(lines, i);
        }
        if lines[next].is_indented() {
            return self.definition_list(lines, i);
        }
        let under = lines[next].text;
        if !is_adornment(under) {
            return self.paragraph(lines, i);
        }
        let title = lines[i];
        let short = column_width(title.text) > under.len();
        if short && under.len() < 4 {
            return self.paragraph(lines, i);
        }
        let span = self.span_of(&lines[i], &lines[next]);
        if !sections {
            self.report_at(SECTION_UNEXPECTED, span);
            return Ok(2);
        }
        if short {
            self.report_at(SECTION_UNDERLINE_SHORT, span);
        }
        self.section(title, Style { over: 0, under: under.as_bytes()[0] }, span)?;
        Ok(2)
    }

    /// Handles a line of repeated punctuation: a transition or the overline of a title.
    /// Lines shorter than four characters fall back to text where Docutils does.
    pub(crate) fn adornment(
        &mut self,
        lines: &[Line<'_>],
        i: usize,
        sections: bool,
    ) -> Result<usize, Fail> {
        let over = lines[i].text;
        let short = over.len() < 4;
        if !sections {
            if over == "::" || short {
                return self.paragraph(lines, i);
            }
            let span = self.line_span(&lines[i]);
            self.report_at(SECTION_UNEXPECTED, span);
            return Ok(1);
        }
        let next = i + 1;
        if next >= lines.len() || lines[next].is_blank() {
            if short {
                return self.paragraph(lines, i);
            }
            self.transition(&lines[i])?;
            return Ok(1);
        }
        let problem = if is_adornment(lines[next].text) {
            Some((SECTION_INVALID, 2))
        } else if next + 1 >= lines.len() {
            Some((SECTION_INCOMPLETE, 2))
        } else {
            None
        };
        if let Some((m, n)) = problem {
            if short {
                return self.paragraph(lines, i);
            }
            let span = self.span_of(&lines[i], &lines[next]);
            self.report_at(m, span);
            return Ok(n);
        }
        self.overlined_title(lines, i, short)
    }

    fn overlined_title(
        &mut self,
        lines: &[Line<'_>],
        i: usize,
        short: bool,
    ) -> Result<usize, Fail> {
        let over = lines[i].text;
        let title = lines[i + 1].trim();
        let under = lines[i + 2].text;
        let span = self.span_of(&lines[i], &lines[i + 2]);
        let problem = if !is_adornment(under) {
            Some(SECTION_NO_UNDERLINE)
        } else if under != over {
            Some(SECTION_MISMATCH)
        } else {
            None
        };
        if let Some(m) = problem {
            if short {
                return self.paragraph(lines, i);
            }
            self.report_at(m, span);
            return Ok(3);
        }
        if column_width(title.text) > over.len() {
            if short {
                return self.paragraph(lines, i);
            }
            self.report_at(SECTION_OVERLINE_SHORT, span);
        }
        let c = over.as_bytes()[0];
        self.section(title, Style { over: c, under: c }, span)?;
        Ok(3)
    }

    /// Opens a section for a title. Its level comes from the order in which adornment
    /// styles first appear; open sections of the same or a deeper level close.
    fn section(&mut self, title: Line<'_>, style: Style, span: SourceSpan) -> Result<(), Fail> {
        let current = self.st.sections.len() - self.st.section_base;
        let mut level = match self.st.styles.iter().position(|&s| s == style) {
            Some(k) if k + 1 > current + 1 => {
                self.report_at(SECTION_LEVEL, span);
                return Ok(());
            }
            Some(k) => k + 1,
            None => {
                if self.st.styles.len() != current || self.st.sections.len() >= MAX_LEVELS {
                    self.report_at(SECTION_LEVEL, span);
                    return Ok(());
                }
                self.st.styles.push(style);
                self.st.styles.len()
            }
        };
        level += self.st.level_base; // within titled content, below the sections around it.
        self.close_sections(level)?;
        let id = self.open(NodeKind::Section, span)?;
        let value = i32::try_from(level).unwrap_or(i32::MAX);
        self.ctx.b.set_value(id, value);
        let h = self.ctx.b.begin_node(NodeKind::Heading, self.line_span(&title))?;
        self.ctx.b.set_value(h, value);
        self.inline(&[title])?;
        self.close(h)?;
        let src = self.st.src;
        self.st.sections.push(SectionEntry { id, level, src, end: span.end });
        self.st.top_count = 0;
        self.st.top_transition = false;
        Ok(())
    }

    /// Closes open sections of `level` or deeper.
    pub(crate) fn close_sections(&mut self, level: usize) -> Result<(), Fail> {
        while let Some(&e) = self.st.sections.last() {
            if e.level < level {
                break;
            }
            self.ctx.b.set_span_end(e.id, e.end);
            self.close(e.id)?;
            self.st.sections.pop();
            if let Some(parent) = self.st.sections.last_mut()
                && parent.src == e.src
            {
                parent.end = parent.end.max(e.end);
            }
        }
        Ok(())
    }

    /// Records a top-level element for transition checks and section spans.
    pub(crate) fn note_top(&mut self, lines: &[Line<'_>], before: usize) {
        if let Some(n) = self.ctx.b.snap.nodes.get(before)
            && n.kind != NodeKind::Section
        {
            self.st.top_count += 1;
            self.st.top_transition = n.kind == NodeKind::ThematicBreak;
        }
        let tab = self.st.tab;
        let src = self.st.src;
        let last = trim_blank_end(lines).last().map(|l| l.end(tab));
        if let (Some(top), Some(end)) = (self.st.sections.last_mut(), last)
            && top.src == src
        {
            top.end = top.end.max(end);
        }
    }

    fn transition(&mut self, l: &Line<'_>) -> Result<(), Fail> {
        let span = self.line_span(l);
        if self.st.top_count == 0 {
            self.report_at(TRANSITION_START, span);
        } else if self.st.top_transition {
            self.report_at(TRANSITION_ADJACENT, span);
        }
        self.leaf(NodeKind::ThematicBreak, span, guidedoc::Text::default())?;
        self.st.transition_span = span;
        Ok(())
    }

    /// Prepares for titled content (`parse_titled`) as Sphinx's only directive does:
    /// when the content's first title has an adornment the document already uses, the
    /// content stands where a title of that level would, so deeper sections close first.
    pub fn place_titled(&mut self, lines: &[Line<'_>]) -> Result<(), Fail> {
        let Some(style) = first_title_style(lines) else {
            return Ok(());
        };
        match self.st.styles.iter().position(|&s| s == style) {
            Some(k) => self.close_sections(self.st.level_base + k + 1),
            None => Ok(()),
        }
    }

    /// Parses content in which titles begin sections, in a title context of its own
    /// (Sphinx's nested parse with titles): adornments take levels afresh, the first
    /// nesting in the innermost open section, and the sections close when the content
    /// ends. The surrounding adornments are kept for what follows.
    pub fn parse_titled(&mut self, lines: &[Line<'_>]) -> Result<(), Fail> {
        let styles = std::mem::take(&mut self.st.styles);
        let (section_base, level_base) = (self.st.section_base, self.st.level_base);
        self.st.section_base = self.st.sections.len();
        self.st.level_base = self.st.sections.last().map_or(0, |s| s.level);
        let r = self.parse_blocks(lines, true, None);
        let r = r.and_then(|()| self.close_sections(self.st.level_base + 1));
        self.st.styles = styles;
        (self.st.section_base, self.st.level_base) = (section_base, level_base);
        r
    }
}

/// The adornment of the first title among lines at their own level, found without
/// parsing them: an underlined line, or a line between an overline and an underline of
/// the same character. Indented lines belong to the blocks above them.
fn first_title_style(lines: &[Line<'_>]) -> Option<Style> {
    let mut i = 0;
    while i < lines.len() {
        if lines[i].is_blank() || lines[i].is_indented() {
            i += 1;
            continue;
        }
        let t = lines[i].text;
        let has_next =
            i + 1 < lines.len() && !lines[i + 1].is_blank() && !lines[i + 1].is_indented();
        match detect(lines, i) {
            Construct::Text if has_next => {
                let under = lines[i + 1].text;
                if is_adornment(under) && !(column_width(t) > under.len() && under.len() < 4) {
                    return Some(Style { over: 0, under: under.as_bytes()[0] });
                }
            }
            Construct::Adornment if has_next => {
                let closed = i + 2 < lines.len() && lines[i + 2].text == t;
                if closed && t.len() >= 4 && !is_adornment(lines[i + 1].text) {
                    let c = t.as_bytes()[0];
                    return Some(Style { over: c, under: c });
                }
            }
            _ => {}
        }
        while i < lines.len() && !lines[i].is_blank() {
            i += 1;
        }
    }
    None
}
