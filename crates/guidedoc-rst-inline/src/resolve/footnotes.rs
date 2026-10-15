//! Footnote numbering and footnote and citation references.

use guidedoc::{Arg, Fail, LinkKind, NodeFlag, NodeFlags, NodeId, Text, arg_static};

use super::{Name, RecordKind, Resolver};
use crate::messages::{FOOTNOTE_TOO_MANY, REFERENCE_UNRESOLVED};
use crate::roles::fold_key;

impl Resolver<'_, '_> {
    /// Gives auto-numbered footnotes, in document order, the lowest numbers that no
    /// footnote, target, or section already uses as a name.
    pub fn number_footnotes(&mut self) -> Result<(), Fail> {
        let mut next = 1;
        for k in 0..self.auto_notes.len() {
            let id = self.auto_notes[k];
            while self.names.contains_key(&next.to_string()) {
                next += 1;
                self.ctx.b.spend(1)?;
            }
            self.ctx.b.set_value(id, next);
            if self.text_of(id) == "#" {
                // An unlabelled footnote is also named by its number, so `3`_ finds it.
                self.add_named(id, RecordKind::Footnote, &next.to_string(), true);
            }
            next += 1;
        }
        for (i, &id) in self.symbol_notes.iter().enumerate() {
            self.ctx.b.set_value(id, i as i32 + 1);
        }
        Ok(())
    }

    /// Returns the next auto-numbered footnote without a label, for `[#]_`.
    fn next_unlabelled(&mut self) -> Option<NodeId> {
        while self.auto_cursor < self.auto_notes.len() {
            let id = self.auto_notes[self.auto_cursor];
            self.auto_cursor += 1;
            if self.text_of(id) == "#" {
                return Some(id);
            }
        }
        None
    }

    /// Matches `[#]_` and `[*]_` in order, and `[1]_` and `[#label]_` by name, then
    /// copies the footnote's number to the reference.
    pub fn resolve_footnote_reference(&mut self, id: NodeId) -> Result<(), Fail> {
        let flags = self.flags_of(id);
        let auto = flags.has(NodeFlag::Auto);
        let label = self.text_of(id).to_owned();
        let target = if flags.has(NodeFlag::Symbol) {
            let Some(&t) = self.symbol_notes.get(self.symbol_used) else {
                self.too_many(id, "symbol");
                return Ok(());
            };
            self.symbol_used += 1;
            t
        } else if auto && label == "#" {
            let Some(t) = self.next_unlabelled() else {
                self.too_many(id, "auto-numbered");
                return Ok(());
            };
            t
        } else {
            let name = if auto { label.get(1..).unwrap_or("") } else { label.as_str() };
            match self.footnote_named(name, auto) {
                Some(t) => t,
                None if !auto && self.footnote_elsewhere(id, name)? => return Ok(()),
                None => {
                    let sp = self.node_span(id);
                    self.report_at(REFERENCE_UNRESOLVED, sp, Arg::None);
                    return Ok(());
                }
            }
        };
        let value = self.ctx.b.snap.nodes[target].value;
        self.ctx.b.set_value(id, value);
        let row = self.ensure_row(id)?;
        self.ctx.b.resolve_link(row, target, LinkKind::Internal, Text::default());
        self.ctx.b.add_flags(target, NodeFlags::REFERENCED);
        Ok(())
    }

    /// Finds a manually numbered footnote, or a labelled auto-numbered one.
    fn footnote_named(&self, label: &str, auto: bool) -> Option<NodeId> {
        let Some(Name::Record(idx)) = self.lookup(&fold_key(label)) else { return None };
        let rec = self.records[idx as usize];
        if rec.kind != RecordKind::Footnote {
            return None;
        }
        let flags = self.flags_of(rec.node);
        // [#label]_ needs a labelled auto-numbered footnote; [n]_ finds a footnote
        // numbered n by hand, or an unlabelled auto-numbered one that got the number n.
        if (auto && !flags.has(NodeFlag::Auto)) || flags.has(NodeFlag::Symbol) {
            return None;
        }
        Some(rec.node)
    }

    /// Reports the first reference without a footnote left, once per kind.
    fn too_many(&mut self, id: NodeId, what: &'static str) {
        let reported = if what == "symbol" { &mut self.symbol_full } else { &mut self.auto_full };
        if *reported {
            return;
        }
        *reported = true;
        let sp = self.node_span(id);
        self.report_at(FOOTNOTE_TOO_MANY, sp, arg_static(what));
    }

    pub fn resolve_citation_reference(&mut self, id: NodeId) -> Result<(), Fail> {
        let key = fold_key(self.text_of(id));
        let found = match self.lookup(&key) {
            Some(Name::Record(idx)) if self.records[idx as usize].kind == RecordKind::Citation => {
                Some(self.records[idx as usize].node)
            }
            _ => None,
        };
        let Some(target) = found else {
            let sp = self.node_span(id);
            self.report_at(REFERENCE_UNRESOLVED, sp, Arg::None);
            return Ok(());
        };
        let row = self.ensure_row(id)?;
        self.ctx.b.resolve_link(row, target, LinkKind::Internal, Text::default());
        self.ctx.b.add_flags(target, NodeFlags::REFERENCED);
        Ok(())
    }

    /// Resolves `[n]_` through any target named n when no footnote has that number,
    /// as reference names are shared by all targets; the reference keeps its label as
    /// its number.
    fn footnote_elsewhere(&mut self, id: NodeId, label: &str) -> Result<bool, Fail> {
        let Some(Name::Record(idx)) = self.lookup(&fold_key(label)) else { return Ok(false) };
        let Some(last) = self.resolve_record(idx)? else { return Ok(false) };
        let n = label
            .bytes()
            .filter(u8::is_ascii_digit)
            .fold(0i64, |n, c| (n * 10 + i64::from(c - b'0')).min(1 << 40));
        self.ctx.b.set_value(id, n.min(1 << 30) as i32);
        let row = self.ensure_row(id)?;
        self.ctx.b.resolve_link(row, last.node, last.kind, last.destination);
        let node = self.records[idx as usize].node;
        self.mark_referenced(node);
        Ok(true)
    }
}
