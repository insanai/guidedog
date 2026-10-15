//! Document structure transforms, after resolution, in the order the specification
//! applies them: the document title and subtitle, section subtitles, bibliographic
//! fields, and transitions that end a section.

use guidedoc::{AttributeKey, Fail, NodeId, NodeKind};

use super::Resolver;
use crate::SECTION_SUBTITLE_CLASS;

impl Resolver<'_, '_> {
    pub fn restructure(&mut self) -> Result<(), Fail> {
        self.trim_reference_space()?;
        if self.ctx.options.promote_title {
            self.promote_titles()?;
        }
        if self.options.sectsubtitle {
            self.section_subtitles()?;
        }
        if self.options.docinfo {
            self.bibliographic_fields()?;
        }
        self.raise_transitions();
        self.smart_quotes()
    }

    /// Elements that may precede the document title and the bibliographic fields
    /// without taking their place: comments, targets, substitution definitions, raw
    /// content, and directives that leave no visible element there.
    pub fn pre_bibliographic(&self, id: NodeId) -> bool {
        match self.kind_of(id) {
            NodeKind::Comment
            | NodeKind::Target
            | NodeKind::SubstitutionDefinition
            | NodeKind::Title
            | NodeKind::Subtitle
            | NodeKind::RawBlock => true,
            NodeKind::Directive => matches!(
                self.attribute(id, AttributeKey::Name),
                Some("title" | "meta" | "header" | "footer" | "sectnum" | "target-notes")
            ),
            _ => false,
        }
    }

    /// Returns the first child of `parent`, from index `from` on, that is not
    /// pre-bibliographic.
    pub fn first_body(&mut self, parent: NodeId, from: usize) -> Result<Option<NodeId>, Fail> {
        let end = self.node_end(parent);
        let mut i = from;
        while i < end {
            self.ctx.b.spend(1)?;
            let id = NodeId::from_index(i);
            if !self.pre_bibliographic(id) {
                return Ok(Some(id));
            }
            i = self.node_end(id);
        }
        Ok(None)
    }

    /// Moves a transition that ends a section up the tree until it follows the
    /// section, as the specification places transitions between sections. Inner
    /// sections come later in document order, so they are handled first.
    fn raise_transitions(&mut self) {
        for i in (1..self.used()).rev() {
            let id = NodeId::from_index(i);
            if self.kind_of(id) != NodeKind::Section {
                continue;
            }
            let Some(last) = self.last_child(id) else { continue };
            let end = self.ctx.b.snap.nodes[id].subtree_end;
            if self.kind_of(last) == NodeKind::ThematicBreak && last.0 + 1 == end {
                self.ctx.b.snap.nodes[i].subtree_end -= 1;
            }
        }
    }

    /// Promotes the title of a section's only subsection to the section's subtitle,
    /// when nothing but pre-bibliographic elements precede it. The subsection's content
    /// becomes the section's, one level up.
    fn section_subtitles(&mut self) -> Result<(), Fail> {
        let mut i = 1;
        while i < self.used() {
            let section = NodeId::from_index(i);
            i += 1;
            if self.kind_of(section) != NodeKind::Section {
                continue;
            }
            let heading = section + 1;
            if heading.index() >= self.used() || self.kind_of(heading) != NodeKind::Heading {
                continue;
            }
            let from = self.node_end(heading);
            let Some(sub) = self.first_body(section, from)? else { continue };
            if self.kind_of(sub) == NodeKind::Section
                && self.node_end(sub) == self.node_end(section)
            {
                self.promote_subtitle(section, sub)?;
            }
        }
        Ok(())
    }

    fn promote_subtitle(&mut self, section: NodeId, sub: NodeId) -> Result<(), Fail> {
        let sub_end = self.node_end(sub);
        self.ctx.b.unwrap_node(sub)?;
        // The subsection's heading now sits at sub; its content follows it.
        let h = sub;
        let h_end = self.node_end(h);
        self.shift_levels(h_end, sub_end - 1, -1);
        let dest = self.node_end(section + 1);
        self.rotate(dest, h.index(), h_end);
        let dest = NodeId::from_index(dest);
        self.ctx.b.set_kind(dest, NodeKind::Paragraph);
        self.ctx.b.set_value(dest, 0);
        self.set_attribute(dest, AttributeKey::Class, SECTION_SUBTITLE_CLASS, "")
    }

    /// Title promotion (Docutils' DocTitle): when the only body element of the
    /// document, apart from comments, targets, substitution definitions, and title or
    /// meta directives, is one section, its title becomes the document Title and its
    /// content moves up one level. Then the same rule may promote a lone subsection to
    /// Subtitle.
    fn promote_titles(&mut self) -> Result<(), Fail> {
        let Some(section) = self.promotion_candidate()? else { return Ok(()) };
        let Some(title) = self.promote(section, NodeKind::Title, NodeId::from_index(1))? else {
            return Ok(());
        };
        let Some(subsection) = self.promotion_candidate()? else { return Ok(()) };
        let dest = NodeId::from_index(self.node_end(title));
        self.promote(subsection, NodeKind::Subtitle, dest)?;
        Ok(())
    }

    /// Returns the section that is the first and last body element.
    fn promotion_candidate(&mut self) -> Result<Option<NodeId>, Fail> {
        let mut i = 1;
        while i < self.used() {
            self.ctx.b.spend(1)?;
            let id = NodeId::from_index(i);
            if !self.pre_bibliographic(id) {
                let lone =
                    self.kind_of(id) == NodeKind::Section && self.node_end(id) == self.used();
                return Ok(lone.then_some(id));
            }
            i = self.node_end(id);
        }
        Ok(None)
    }

    /// Turns the section's heading into `kind`, removes the section node, moves the
    /// heading to `dest`, and raises every remaining section by one level.
    fn promote(
        &mut self,
        section: NodeId,
        kind: NodeKind,
        dest: NodeId,
    ) -> Result<Option<NodeId>, Fail> {
        let h = section + 1;
        if h.index() >= self.used() || self.kind_of(h) != NodeKind::Heading {
            return Ok(None);
        }
        self.ctx.b.spend(4 * self.used() as u64)?;
        self.ctx.b.set_kind(h, kind);
        self.ctx.b.set_value(h, 0);
        self.ctx.b.unwrap_node(section)?;
        let title_end = self.ctx.b.snap.nodes[section].subtree_end as usize;
        self.rotate(dest.index(), section.index(), title_end);
        for node in self.ctx.b.snap.nodes.iter_mut().skip(1) {
            if matches!(node.kind, NodeKind::Section | NodeKind::Heading) && node.value > 1 {
                node.value -= 1;
            }
        }
        Ok(Some(dest))
    }

    /// Removes the white space before footnote and citation references
    /// (`Options::trim_footnote_reference_space`), so "text [1]_" reads "text[1]".
    fn trim_reference_space(&mut self) -> Result<(), Fail> {
        if !self.options.trim_footnote_reference_space {
            return Ok(());
        }
        let mut i = self.used();
        while i > 2 {
            i -= 1;
            let k = self.kind_of(NodeId::from_index(i));
            if k != NodeKind::FootnoteReference && k != NodeKind::CitationReference {
                continue;
            }
            let mut at = i;
            while at >= 2 && self.kind_of(NodeId::from_index(at - 1)) == NodeKind::SoftBreak {
                self.remove_range(at - 1, at)?;
                at -= 1;
            }
            let before = NodeId::from_index(at - 1);
            if self.kind_of(before) == NodeKind::Text {
                let kept = self.text_of(before).trim_end_matches([' ', '\t']).len();
                self.ctx.b.snap.nodes[before.index()].text.len = kept as u32;
                if kept == 0 {
                    self.remove_range(before.index(), before.index() + 1)?;
                    at -= 1;
                }
            }
            i = at;
        }
        Ok(())
    }
}
