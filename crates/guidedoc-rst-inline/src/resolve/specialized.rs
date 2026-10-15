//! The specialized roles name inline constructs explicitly: `:named-reference:` is
//! `name_`, `:anonymous-reference:` is `` `text`__ ``, `:footnote-reference:` is
//! `[1]_`, `:citation-reference:` is `[CIT]_`, `:substitution-reference:` is `|name|`,
//! `:target:` is `` _`text` ``, and `:uri-reference:` is a hyperlink to the URI.
//! Parsing makes them Role nodes without a class; this pass, before resolution, gives
//! them the construct's node kind and link row.

use guidedoc::{AttributeKey, Fail, LinkInfo, LinkKind, NodeFlags, NodeId, NodeKind, Text};

use super::Resolver;
use crate::markup::unresolved;
use crate::roles::fold_key;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Specialized {
    Named,
    Anonymous,
    Footnote,
    Citation,
    Substitution,
    Target,
    Uri,
}

fn specialized_kind(name: &str) -> Option<Specialized> {
    Some(match name {
        "named-reference" => Specialized::Named,
        "anonymous-reference" => Specialized::Anonymous,
        "footnote-reference" => Specialized::Footnote,
        "citation-reference" => Specialized::Citation,
        "substitution-reference" => Specialized::Substitution,
        "target" => Specialized::Target,
        "uri-reference" => Specialized::Uri,
        _ => return None,
    })
}

impl Resolver<'_, '_> {
    pub fn specialize_roles(&mut self) -> Result<(), Fail> {
        for i in (1..self.used()).rev() {
            let id = NodeId::from_index(i);
            if self.kind_of(id) != NodeKind::Role
                || self.attribute(id, AttributeKey::Class).is_some()
            {
                continue;
            }
            if let Some(which) = specialized_kind(self.text_of(id)) {
                self.specialize(id, which)?;
            }
        }
        Ok(())
    }

    fn specialize(&mut self, id: NodeId, which: Specialized) -> Result<(), Fail> {
        let content = self.role_content(id);
        let name = self.ctx.b.add_text(&fold_key(&content))?;
        let b = &mut self.ctx.b;
        match which {
            Specialized::Named | Specialized::Anonymous => {
                b.set_kind(id, NodeKind::Reference);
                b.set_text(id, Text::default());
                if which == Specialized::Anonymous {
                    b.add_flags(id, NodeFlags::ANONYMOUS);
                }
                let name = if which == Specialized::Named { name } else { Text::default() };
                b.set_link(id, unresolved(name))?;
            }
            Specialized::Target => {
                b.set_kind(id, NodeKind::InlineTarget);
                b.set_text(id, name);
            }
            Specialized::Uri => {
                let start = b.text_open();
                for c in content.bytes().filter(|c| !matches!(c, b' ' | b'\n' | b'\t')) {
                    b.text_push_byte(c)?;
                }
                let dest = b.text_close(start);
                b.set_kind(id, NodeKind::Link);
                b.set_text(id, Text::default());
                let link = LinkInfo {
                    destination: dest,
                    kind: LinkKind::External,
                    ..LinkInfo::UNRESOLVED
                };
                b.set_link(id, link)?;
            }
            Specialized::Footnote | Specialized::Citation | Specialized::Substitution => {
                self.specialize_label(id, which, &content, name)?;
            }
        }
        Ok(())
    }

    /// Footnote, citation, and substitution references keep the content as a label and
    /// lose their children.
    fn specialize_label(
        &mut self,
        id: NodeId,
        which: Specialized,
        content: &str,
        name: Text,
    ) -> Result<(), Fail> {
        let label_text = content.trim_matches([' ', '\n', '\t']);
        let label = self.ctx.b.add_text(label_text)?;
        let end = self.node_end(id);
        self.remove_range(id.index() + 1, end)?;
        let b = &mut self.ctx.b;
        b.set_text(id, label);
        match which {
            Specialized::Footnote => {
                b.set_kind(id, NodeKind::FootnoteReference);
                if label_text == "*" {
                    b.add_flags(id, NodeFlags::SYMBOL);
                } else if label_text.starts_with('#') {
                    b.add_flags(id, NodeFlags::AUTO);
                }
            }
            Specialized::Citation => b.set_kind(id, NodeKind::CitationReference),
            _ => {
                b.set_kind(id, NodeKind::SubstitutionReference);
                b.set_link(id, unresolved(name))?;
            }
        }
        Ok(())
    }

    /// The text of a role's Text children.
    fn role_content(&self, id: NodeId) -> String {
        let mut out = String::new();
        for i in id.index() + 1..self.node_end(id) {
            let child = NodeId::from_index(i);
            if self.kind_of(child) == NodeKind::Text {
                out.push_str(self.text_of(child));
            }
        }
        out
    }
}
