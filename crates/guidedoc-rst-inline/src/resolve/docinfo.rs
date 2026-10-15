//! Bibliographic fields. A field list that is the document's first body element
//! (after the title, subtitle, and pre-bibliographic elements) holds the document's
//! bibliographic data. It moves after the title and subtitle and gets class "docinfo".
//! Fields the document language names (author, authors, organization, address,
//! contact, version, revision, status, date, copyright) get Custom attribute "docinfo"
//! = the field's English key; "dedication" and "abstract" become topics with that
//! class after the list. Other fields get a class made from their name. RCS keywords
//! ("$Date: 2002/08/27 13:04:05 $") are reduced to their values.

use guidedoc::{Arg, AttributeKey, Fail, NodeId, NodeKind, ROOT, make_id};

use super::Resolver;
use crate::DOCINFO_CLASS;
use crate::languages::{Bibliographic, Language, language_of};
use crate::messages::{DOCINFO_AUTHORS, DOCINFO_EMPTY, DOCINFO_PARAGRAPH, DOCINFO_REPEATED};
use crate::roles::fold_key;

impl Resolver<'_, '_> {
    pub fn bibliographic_fields(&mut self) -> Result<(), Fail> {
        let Some(list) = self.first_body(ROOT, 1)? else { return Ok(()) };
        if self.kind_of(list) != NodeKind::FieldList {
            return Ok(());
        }
        let list = self.place_docinfo(list);
        self.set_attribute(list, AttributeKey::Class, DOCINFO_CLASS, "")?;
        let lang = language_of(&self.options.language);
        let mut moved = 0;
        for topic in [Bibliographic::Dedication, Bibliographic::Abstract] {
            if let Some(field) = self.find_field(list, lang, topic) {
                moved += self.make_topic(list, field, lang, topic, moved)?;
            }
        }
        let mut i = list.index() + 1;
        while i < self.node_end(list) {
            self.classify_field(NodeId::from_index(i), lang)?;
            i = self.node_end(NodeId::from_index(i));
        }
        // A list whose fields all became topics disappears.
        if self.node_end(list) == list.index() + 1 {
            self.remove_range(list.index(), list.index() + 1)?;
        }
        Ok(())
    }

    /// Moves the list to follow the title and subtitle.
    fn place_docinfo(&mut self, list: NodeId) -> NodeId {
        let mut dest = 1;
        while dest < list.index() {
            let k = self.kind_of(NodeId::from_index(dest));
            if k != NodeKind::Title && k != NodeKind::Subtitle {
                break;
            }
            dest = self.node_end(NodeId::from_index(dest));
        }
        if dest < list.index() {
            let end = self.node_end(list);
            self.rotate(dest, list.index(), end);
        }
        NodeId::from_index(dest)
    }

    /// The field's name, case-folded with white space normalized.
    fn field_key(&self, field: NodeId) -> String {
        let mut plain = String::new();
        self.ctx.b.document().plain_text(field + 1, &mut plain);
        fold_key(&plain)
    }

    fn find_field(&self, list: NodeId, lang: &Language, want: Bibliographic) -> Option<NodeId> {
        let mut i = list.index() + 1;
        while i < self.node_end(list) {
            let field = NodeId::from_index(i);
            if lang.bibliographic(&self.field_key(field)) == Some(want) {
                return (self.body_count(field) > 0).then_some(field);
            }
            i = self.node_end(field);
        }
        None
    }

    pub fn field_body(&self, field: NodeId) -> NodeId {
        NodeId::from_index(self.node_end(field + 1))
    }

    pub fn body_count(&self, field: NodeId) -> usize {
        let body = self.field_body(field);
        let mut n = 0;
        let mut c = body.index() + 1;
        while c < self.node_end(body) {
            n += 1;
            c = self.node_end(NodeId::from_index(c));
        }
        n
    }

    /// Turns a dedication or abstract field into a topic that follows the list and the
    /// topics moved before it. Returns the topic's size.
    fn make_topic(
        &mut self,
        list: NodeId,
        field: NodeId,
        lang: &Language,
        topic: Bibliographic,
        moved: usize,
    ) -> Result<usize, Fail> {
        let name = field + 1;
        self.ctx.b.set_kind(name, NodeKind::Title);
        let label =
            if topic == Bibliographic::Dedication { lang.dedication } else { lang.abstract_ };
        if self.node_end(name) == name.index() + 2 && self.kind_of(name + 1) == NodeKind::Text {
            let t = self.ctx.b.add_text(label)?;
            self.ctx.b.set_text(name + 1, t);
        }
        self.ctx.b.unwrap_node(self.field_body(field))?;
        self.ctx.b.set_kind(field, NodeKind::Topic);
        self.set_attribute(field, AttributeKey::Class, topic.key(), "")?;
        let size = self.node_end(field) - field.index();
        self.move_out(list, field.index(), field.index() + size, moved)?;
        Ok(size)
    }

    fn classify_field(&mut self, field: NodeId, lang: &Language) -> Result<(), Fail> {
        let key = self.field_key(field);
        let kind = lang.bibliographic(&key);
        let name_span = self.node_span(field + 1);
        let count = self.body_count(field);
        let first = self.field_body(field) + 1;
        let ok = match kind {
            None => false,
            Some(_) if count == 0 => {
                self.report_at(DOCINFO_EMPTY, name_span, Arg::None);
                false
            }
            Some(Bibliographic::Dedication | Bibliographic::Abstract) => {
                self.report_at(DOCINFO_REPEATED, name_span, Arg::None);
                false
            }
            Some(Bibliographic::Authors) => {
                let ok = self.extract_authors(field, lang)?;
                if !ok {
                    self.report_at(DOCINFO_AUTHORS, name_span, Arg::None);
                }
                ok
            }
            Some(other) => {
                let ok = count == 1 && self.kind_of(first) == NodeKind::Paragraph;
                if !ok {
                    self.report_at(DOCINFO_PARAGRAPH, name_span, Arg::None);
                } else {
                    self.clean_keywords(first)?;
                    if other == Bibliographic::Address {
                        self.ctx.b.set_kind(first, NodeKind::Preformatted);
                    }
                }
                ok
            }
        };
        if let (true, Some(kind)) = (ok, kind) {
            return self.set_attribute(field, AttributeKey::Custom, kind.key(), DOCINFO_CLASS);
        }
        let mut id = String::new();
        make_id(&mut id, &key);
        if !id.is_empty() {
            self.set_attribute(field, AttributeKey::Class, &id, "")?;
        }
        if count == 1 && self.kind_of(first) == NodeKind::Paragraph {
            self.clean_keywords(first)?;
        }
        Ok(())
    }
}
