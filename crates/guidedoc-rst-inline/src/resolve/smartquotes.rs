//! Smart quotes (`Options::smart_quotes`): straight quotation marks become the
//! document language's typographic ones, "--" an en dash, "---" an em dash, and "..."
//! an ellipsis. Each element with inline content is one stream of text: inline
//! literals, math, and raw text are not changed but give context, and so do line
//! breaks. A class `language-<tag>` on an element or an ancestor selects that language.
//! Literal blocks, comments, raw content, and option strings keep their text. Escaped
//! quotation marks stay straight.

use guidedoc::{AttributeKey, Fail, NodeId, NodeKind, decode_rune};

use super::Resolver;
use crate::SmartQuotes;
use crate::languages::{Language, language_of, tag_equal};

/// The state of one element's text stream.
pub(crate) struct QuoteContext {
    pub quotes: [String; 4],
    pub english: bool,
    /// The last character before the current text; '\0' at the start.
    pub prev: char,
}

impl Resolver<'_, '_> {
    pub fn smart_quotes(&mut self) -> Result<(), Fail> {
        if self.options.smart_quotes == SmartQuotes::Off {
            return Ok(());
        }
        let base = language_of(&self.options.language);
        let mut scopes: Vec<(usize, &'static Language)> = Vec::new();
        let mut i = 1;
        while i < self.used() {
            while scopes.last().is_some_and(|s| s.0 <= i) {
                scopes.pop();
            }
            let id = NodeId::from_index(i);
            let mut lang = scopes.last().map_or(base, |s| s.1);
            if let Some(l) = self.class_language(id) {
                lang = l;
                if scopes.len() < 64 {
                    scopes.push((self.node_end(id), l));
                }
            }
            match self.kind_of(id) {
                NodeKind::CodeBlock
                | NodeKind::DoctestBlock
                | NodeKind::MathBlock
                | NodeKind::RawBlock
                | NodeKind::Comment
                | NodeKind::Preformatted
                | NodeKind::Target
                | NodeKind::Option
                | NodeKind::OptionArgument
                | NodeKind::Directive => i = self.node_end(id),
                NodeKind::Title
                | NodeKind::Subtitle
                | NodeKind::Heading
                | NodeKind::Paragraph
                | NodeKind::Attribution
                | NodeKind::Term
                | NodeKind::Classifier
                | NodeKind::FieldName
                | NodeKind::Line
                | NodeKind::Caption
                | NodeKind::Rubric
                | NodeKind::SubstitutionDefinition => {
                    self.educate_element(id, lang)?;
                    i = self.node_end(id);
                }
                _ => i += 1,
            }
        }
        Ok(())
    }

    /// Finds a class `language-<tag>` of a node.
    fn class_language(&self, id: NodeId) -> Option<&'static Language> {
        let snap = &*self.ctx.b.snap;
        let attrs = snap.attributes.get(snap.nodes[id].attributes.range())?;
        attrs
            .iter()
            .filter(|a| a.key == AttributeKey::Class)
            .flat_map(|a| snap.pool_str(a.value).split(' '))
            .find_map(|c| c.strip_prefix("language-").filter(|t| !t.is_empty()))
            .map(language_of)
    }

    fn educate_element(&mut self, e: NodeId, lang: &'static Language) -> Result<(), Fail> {
        let set =
            if self.options.smart_quotes == SmartQuotes::Alt { lang.alt } else { lang.quotes };
        let mut quotes = set.map(str::to_owned);
        locale_quotes(&self.options.smart_quotes_locales, lang.tag, &mut quotes);
        let mut qc = QuoteContext { quotes, english: lang.tag == "en", prev: '\0' };
        let end = self.node_end(e);
        let mut i = e.index() + 1;
        while i < end {
            let id = NodeId::from_index(i);
            match self.kind_of(id) {
                NodeKind::Text => {
                    let next = self.next_char(i + 1, end);
                    self.educate_text(id, &mut qc, next)?;
                }
                NodeKind::Code | NodeKind::Math | NodeKind::RawInline => {
                    if let Some(last) = self.text_of(id).chars().next_back() {
                        qc.prev = last;
                    }
                }
                NodeKind::SoftBreak | NodeKind::HardBreak => qc.prev = ' ',
                NodeKind::FootnoteReference | NodeKind::CitationReference => qc.prev = 'x',
                NodeKind::Strong | NodeKind::Emphasis
                    if self.custom_attribute(id, "literal") == Some("1") =>
                {
                    i = self.skip_literal(i, &mut qc);
                }
                _ => {}
            }
            i += 1;
        }
        Ok(())
    }

    /// Passes over Sphinx's literal_strong or literal_emphasis at `i`, whose text stays
    /// as written but gives context; returns its last node.
    fn skip_literal(&self, i: usize, qc: &mut QuoteContext) -> usize {
        let end = self.node_end(NodeId::from_index(i));
        for k in i + 1..end {
            let id = NodeId::from_index(k);
            if self.kind_of(id) == NodeKind::Text
                && let Some(last) = self.text_of(id).chars().next_back()
            {
                qc.prev = last;
            }
        }
        end - 1
    }

    /// The first character of the text that follows position `from`.
    fn next_char(&self, from: usize, end: usize) -> char {
        for i in from..end {
            let id = NodeId::from_index(i);
            match self.kind_of(id) {
                NodeKind::Text | NodeKind::Code | NodeKind::Math | NodeKind::RawInline => {
                    if let Some(c) = self.text_of(id).chars().next() {
                        return c;
                    }
                }
                NodeKind::SoftBreak | NodeKind::HardBreak => return ' ',
                NodeKind::FootnoteReference | NodeKind::CitationReference => return 'x',
                _ => {}
            }
        }
        '\0'
    }
}

/// Applies a "tag:“”‘’,tag:..." override for the language, if listed.
fn locale_quotes(locales: &str, tag: &str, quotes: &mut [String; 4]) {
    for item in locales.split(',') {
        let Some((key, marks)) = item.split_once(':') else { continue };
        if !tag_equal(key.trim_matches([' ', '\n', '\t']), tag) {
            continue;
        }
        let marks = marks.trim_matches([' ', '\n', '\t']);
        let mut k = 0;
        for q in quotes.iter_mut() {
            if k >= marks.len() {
                return;
            }
            let (_, w) = decode_rune(marks, k);
            *q = marks[k..k + w].to_owned();
            k += w;
        }
    }
}
