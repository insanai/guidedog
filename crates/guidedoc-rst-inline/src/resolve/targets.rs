//! Collecting targets and following indirect ones.

use guidedoc::{AttributeKey, Fail, LinkKind, NodeId, NodeKind, arg_span};

use super::{Final, Name, Record, RecordKind, RecordState, Resolver};
use crate::TARGET_NAME_ATTRIBUTE;
use crate::languages::language_of;
use crate::messages::{
    TARGET_AMBIGUOUS, TARGET_CYCLE, TARGET_DUPLICATE, TARGET_IMPLICIT, TARGET_REPEATED,
    TARGET_UNRESOLVED,
};
use crate::roles::fold_key;

impl Resolver<'_, '_> {
    /// Walks the document once, recording every target, footnote, citation, and
    /// substitution definition. An internal target resolves to the next element in
    /// document order that is not a target, comment, or substitution definition;
    /// chained targets share it.
    pub fn collect(&mut self) -> Result<(), Fail> {
        let mut skip_end = 0;
        for i in 1..self.used() {
            self.ctx.b.spend(1)?;
            let id = NodeId::from_index(i);
            let kind = self.kind_of(id);
            if i >= skip_end {
                self.propagate_pending(id, kind);
            }
            match kind {
                NodeKind::Target => self.collect_target(id),
                NodeKind::InlineTarget => {
                    let name = self.text_of(id).to_owned();
                    self.add_named(id, RecordKind::InlineTarget, &name, true);
                }
                NodeKind::Link => self.collect_link(id),
                NodeKind::Reference => self.collect_reference(id),
                NodeKind::Section => {
                    let name = self.section_name(id);
                    self.add_named(id, RecordKind::Section, &name, false);
                }
                NodeKind::Contents => self.contents_name(id),
                NodeKind::Footnote => self.collect_footnote(id),
                NodeKind::Citation => {
                    let name = fold_key(self.text_of(id));
                    self.add_named(id, RecordKind::Citation, &name, true);
                }
                NodeKind::SubstitutionDefinition => {
                    skip_end = self.node_end(id);
                    self.add_definition(id);
                }
                _ => {}
            }
            if let Some(name) = self.element_name(id) {
                self.add_named(id, RecordKind::Named, &fold_key(&name), true);
            }
        }
        let pending = std::mem::take(&mut self.pending);
        for &idx in &pending {
            let rec = &mut self.records[idx as usize];
            rec.last = Final::internal(rec.node);
            rec.state = RecordState::Done;
        }
        self.pending = pending;
        self.pending.clear();
        Ok(())
    }

    fn collect_link(&mut self, id: NodeId) {
        let Some((l, _)) = self.row(id) else { return };
        if l.name.len == 0 {
            return;
        }
        // The destination is known before the name, so a repeated URI agrees.
        let idx = self.add_record(id, RecordKind::Link, true);
        self.records[idx as usize].last =
            Final { kind: l.kind, destination: l.destination, node: guidedoc::INVALID_NODE };
        let name = self.ctx.b.pool_string(l.name).to_owned();
        self.add_name(&name, idx);
    }

    fn collect_reference(&mut self, id: NodeId) {
        if self.flags_of(id).has(guidedoc::NodeFlag::Anonymous) {
            self.anon_refs.push(id);
        }
        if let Some(name) = self.custom_attribute(id, TARGET_NAME_ATTRIBUTE) {
            // An embedded alias names an implicit target, as in Docutils.
            let name = name.to_owned();
            let idx = self.add_record(id, RecordKind::Alias, false);
            self.records[idx as usize].state = RecordState::Unvisited;
            self.add_name(&name, idx);
        }
    }

    /// Settles internal targets waiting for the next element: an element takes them;
    /// another block target takes them into its own chain; comments, substitution
    /// definitions, footnotes, and citations do not take them, so they lead to
    /// themselves, as the specification propagates targets.
    fn propagate_pending(&mut self, id: NodeId, kind: NodeKind) {
        if self.pending.is_empty() {
            return;
        }
        let next = self.records.len() as u32; // collect_target records id next.
        for k in 0..self.pending.len() {
            let rec = &mut self.records[self.pending[k] as usize];
            match kind {
                NodeKind::Target => {
                    rec.via = Some(next);
                    rec.state = RecordState::Unvisited;
                }
                NodeKind::Comment
                | NodeKind::SubstitutionDefinition
                | NodeKind::Footnote
                | NodeKind::Citation => {
                    rec.last = Final::internal(rec.node);
                    rec.state = RecordState::Done;
                }
                _ => {
                    rec.last = Final::internal(id);
                    rec.state = RecordState::Done;
                }
            }
        }
        self.pending.clear();
    }

    pub fn add_record(&mut self, id: NodeId, kind: RecordKind, explicit: bool) -> u32 {
        self.records.push(Record {
            node: id,
            kind,
            explicit,
            state: RecordState::Done,
            last: Final::internal(id),
            via: None,
        });
        (self.records.len() - 1) as u32
    }

    /// Records a target that leads to its own node.
    pub fn add_named(&mut self, id: NodeId, kind: RecordKind, name: &str, explicit: bool) -> u32 {
        let idx = self.add_record(id, kind, explicit);
        self.add_name(name, idx);
        idx
    }

    fn collect_target(&mut self, id: NodeId) {
        let idx = self.add_record(id, RecordKind::Target, true);
        let row = self.row(id);
        let rec = &mut self.records[idx as usize];
        match row {
            Some((l, _)) if l.destination.len > 0 => {
                let kind = if l.kind == LinkKind::Email { l.kind } else { LinkKind::External };
                rec.last =
                    Final { kind, destination: l.destination, node: guidedoc::INVALID_NODE };
            }
            Some((l, _)) if l.kind == LinkKind::Internal => rec.last = Final::internal(l.resolved),
            Some((l, _)) if l.name.len > 0 => rec.state = RecordState::Unvisited,
            _ => {
                rec.state = RecordState::Visiting; // settled by the next element.
                self.pending.push(idx);
            }
        }
        if self.flags_of(id).has(guidedoc::NodeFlag::Anonymous) {
            self.anon_targets.push(idx);
        } else {
            let name = self.text_of(id).to_owned();
            self.add_name(&name, idx);
        }
    }

    fn collect_footnote(&mut self, id: NodeId) {
        let flags = self.flags_of(id);
        let label = self.text_of(id).to_owned();
        if flags.has(guidedoc::NodeFlag::Symbol) {
            self.symbol_notes.push(id);
            self.add_record(id, RecordKind::Footnote, true);
        } else if flags.has(guidedoc::NodeFlag::Auto) {
            self.auto_notes.push(id);
            let name = label.strip_prefix('#').unwrap_or(&label);
            self.add_named(id, RecordKind::Footnote, &fold_key(name), true);
        } else {
            self.add_named(id, RecordKind::Footnote, &fold_key(&label), true);
        }
    }

    /// The normalized plain text of the section's title.
    fn section_name(&self, id: NodeId) -> String {
        let h = id + 1;
        if h.index() >= self.used() || self.kind_of(h) != NodeKind::Heading {
            return String::new();
        }
        let mut plain = String::new();
        self.title_text(h, &mut plain);
        fold_key(&plain)
    }

    /// Gives a record a name, settling duplicates as Docutils does
    /// (`document.set_duplicate_name_id`): a target referring where the name's holder
    /// refers (the same URI, or the same indirect name) is only noted and the holder
    /// keeps the name; otherwise an explicit name wins over implicit ones (section
    /// titles, embedded aliases), and two explicit or two implicit names make the name
    /// unusable. Once an explicit name is unusable it stays so.
    pub fn add_name(&mut self, name: &str, idx: u32) {
        if name.is_empty() {
            return;
        }
        let Some(&existing) = self.names.get(name) else {
            self.names.insert(name.to_owned(), Name::Record(idx));
            return;
        };
        let new = self.records[idx as usize];
        let a = self.quote_span(new.node);
        let old_idx = match existing {
            Name::Ambiguous => {
                if new.explicit {
                    self.names.insert(name.to_owned(), Name::Record(idx));
                }
                return;
            }
            Name::AmbiguousExplicit => {
                if new.explicit && new.kind != RecordKind::Link {
                    self.ctx.report(TARGET_AMBIGUOUS, a, arg_span(a), guidedoc::Arg::None);
                }
                return;
            }
            Name::Record(old) => old,
        };
        let old = self.records[old_idx as usize];
        // An explicit target just before a section names that section: nothing to say.
        if same_element(&old, &new) && old.explicit != new.explicit {
            if new.explicit {
                self.names.insert(name.to_owned(), Name::Record(idx));
            }
            return;
        }
        let b = self.quote_span(old.node);
        let (m, at, first, second) = if self.same_destination(&old, &new) {
            (TARGET_REPEATED, a, a, b)
        } else if new.explicit && old.explicit {
            // Neither name can be used; names from embedded URIs are only noted.
            self.names.insert(name.to_owned(), Name::AmbiguousExplicit);
            let both_links = new.kind == RecordKind::Link && old.kind == RecordKind::Link;
            (if both_links { TARGET_IMPLICIT } else { TARGET_DUPLICATE }, a, a, b)
        } else if new.explicit {
            self.names.insert(name.to_owned(), Name::Record(idx));
            (TARGET_IMPLICIT, b, b, a)
        } else if old.explicit {
            (TARGET_IMPLICIT, a, a, b)
        } else {
            self.names.insert(name.to_owned(), Name::Ambiguous);
            (TARGET_IMPLICIT, a, a, b)
        };
        self.ctx.report(m, at, arg_span(first), arg_span(second));
    }

    /// Two targets naming one URI, or one indirect name, are not a conflict (Docutils
    /// compares refuri and refname).
    fn same_destination(&self, a: &Record, b: &Record) -> bool {
        if let (Some(x), Some(y)) = (self.indirect_name(a), self.indirect_name(b)) {
            return x == y;
        }
        if a.state != RecordState::Done || b.state != RecordState::Done {
            return false;
        }
        if a.last.kind == LinkKind::Internal || b.last.kind == LinkKind::Internal {
            return false;
        }
        if a.last.destination.len == 0 {
            return false;
        }
        self.ctx.b.pool_string(a.last.destination) == self.ctx.b.pool_string(b.last.destination)
    }

    /// The name an indirect target or embedded alias refers to.
    fn indirect_name(&self, rec: &Record) -> Option<&str> {
        if rec.kind != RecordKind::Target && rec.kind != RecordKind::Alias {
            return None;
        }
        let (l, _) = self.row(rec.node)?;
        if l.destination.len > 0 || l.kind == LinkKind::Internal || l.name.len == 0 {
            return None;
        }
        Some(self.ctx.b.pool_string(l.name))
    }

    /// Returns the record an indirect or chained target leads to.
    fn follow(&self, rec: &Record) -> Option<Name> {
        if let Some(via) = rec.via {
            return ((via as usize) < self.records.len()).then_some(Name::Record(via));
        }
        self.lookup(self.indirect_name(rec).unwrap_or(""))
    }

    /// Follows indirect targets to their final destination. Chains are bounded: every
    /// step marks a record, and meeting a marked record is a cycle.
    pub fn resolve_record(&mut self, first: u32) -> Result<Option<Final>, Fail> {
        let mut result = None;
        let mut i = first;
        loop {
            let rec = self.records[i as usize];
            if rec.state == RecordState::Done {
                result = Some(rec.last);
                break;
            }
            if rec.state == RecordState::Failed {
                break;
            }
            self.ctx.b.spend(1)?;
            if rec.state == RecordState::Visiting {
                let sp = self.quote_span(rec.node);
                self.report_at(TARGET_CYCLE, sp, guidedoc::Arg::None);
                break;
            }
            self.records[i as usize].state = RecordState::Visiting;
            let Some(Name::Record(next)) = self.follow(&rec) else {
                let sp = self.quote_span(rec.node);
                self.report_at(TARGET_UNRESOLVED, sp, guidedoc::Arg::None);
                break;
            };
            i = next;
        }
        self.settle_chain(first, result);
        Ok(result)
    }

    /// Marks every record of the chain from `first` done (or failed) with `result`.
    fn settle_chain(&mut self, first: u32, result: Option<Final>) {
        let mut i = first;
        while self.records[i as usize].state == RecordState::Visiting {
            let rec = &mut self.records[i as usize];
            rec.state = if result.is_some() { RecordState::Done } else { RecordState::Failed };
            if let Some(last) = result {
                rec.last = last;
            }
            let rec = *rec;
            let Some(Name::Record(next)) = self.follow(&rec) else { break };
            i = next;
        }
    }

    /// Settles every hyperlink target, so internal and indirect targets carry their
    /// destination even when nothing refers to them. Embedded aliases are settled
    /// here too, before their own reference's link row is overwritten.
    pub fn resolve_targets(&mut self) -> Result<(), Fail> {
        for idx in 0..self.records.len() {
            let rec = self.records[idx];
            if rec.kind != RecordKind::Target && rec.kind != RecordKind::Alias {
                continue;
            }
            let Some(last) = self.resolve_record(idx as u32)? else { continue };
            if rec.kind == RecordKind::Alias {
                continue;
            }
            let row = self.row(rec.node);
            let p = match row {
                Some((l, _)) if last.kind != LinkKind::Internal && l.destination.len > 0 => {
                    continue;
                }
                Some((_, p)) => p,
                None => self.ctx.b.set_link(rec.node, guidedoc::LinkInfo::UNRESOLVED)?,
            };
            self.ctx.b.resolve_link(p, last.node, last.kind, last.destination);
        }
        Ok(())
    }

    /// Writes a title's text as its name is made: the text of text, literal, math, and
    /// raw nodes, and the name of a substitution reference as written, since names
    /// come from the title before substitutions apply.
    pub fn title_text(&self, h: NodeId, out: &mut String) {
        for i in h.index() + 1..self.node_end(h) {
            let id = NodeId::from_index(i);
            match self.kind_of(id) {
                NodeKind::Text
                | NodeKind::Code
                | NodeKind::Math
                | NodeKind::RawInline
                | NodeKind::SubstitutionReference => out.push_str(self.text_of(id)),
                NodeKind::SoftBreak | NodeKind::HardBreak => out.push(' '),
                _ => {}
            }
        }
    }

    /// Gives a table of contents an implicit name, its title or the language's word
    /// for contents, when no earlier element has that name.
    fn contents_name(&mut self, id: NodeId) {
        let title = match self.attribute(id, AttributeKey::Title) {
            Some(t) => t.to_owned(),
            None => language_of(&self.options.language).contents.to_owned(),
        };
        let name = fold_key(&title);
        if self.lookup(&name).is_some() {
            return;
        }
        self.add_named(id, RecordKind::Section, &name, false);
    }
}

/// An explicit internal target and a section title lead to one node.
fn same_element(a: &Record, b: &Record) -> bool {
    let lead = |rec: &Record| {
        (rec.state == RecordState::Done && rec.last.kind == LinkKind::Internal)
            .then_some(rec.last.node)
    };
    matches!((lead(a), lead(b)), (Some(x), Some(y)) if x == y && x != guidedoc::INVALID_NODE)
}
