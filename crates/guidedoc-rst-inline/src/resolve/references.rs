//! Hyperlink references, named and anonymous.

use guidedoc::{Arg, Fail, NodeFlag, NodeId, NodeKind, arg_int};

use super::{Edge, Name, Resolver};
use crate::messages::{ANONYMOUS_MISMATCH, REFERENCE_AMBIGUOUS, REFERENCE_UNRESOLVED};

impl Resolver<'_, '_> {
    /// Resolves every reference in document order. Substitution references inside a
    /// definition become edges for the cycle check.
    pub fn resolve_references(&mut self) -> Result<(), Fail> {
        let mut current: Option<usize> = None;
        let mut def_end = 0;
        for i in 1..self.used() {
            self.ctx.b.spend(1)?;
            let id = NodeId::from_index(i);
            match self.kind_of(id) {
                NodeKind::SubstitutionDefinition => {
                    let k = current.map_or(0, |c| c + 1);
                    current = Some(k);
                    def_end = self.node_end(id);
                    if let Some(def) = self.defs.get_mut(k) {
                        def.first_edge = self.edges.len();
                    }
                }
                NodeKind::Reference if !self.flags_of(id).has(NodeFlag::Anonymous) => {
                    self.resolve_named(id)?;
                }
                NodeKind::FootnoteReference => self.resolve_footnote_reference(id)?,
                NodeKind::CitationReference => self.resolve_citation_reference(id)?,
                NodeKind::SubstitutionReference => {
                    let to = self.resolve_substitution(id)?;
                    if let (Some(to), Some(c)) = (to, current)
                        && i < def_end
                        && c < self.defs.len()
                    {
                        self.edges.push(Edge { to, at: id });
                        self.defs[c].edge_count += 1;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Resolves a named hyperlink reference through the target names.
    fn resolve_named(&mut self, id: NodeId) -> Result<(), Fail> {
        let Some((l, p)) = self.row(id) else { return Ok(()) };
        if l.kind != guidedoc::LinkKind::Unresolved {
            return Ok(());
        }
        let sp = self.node_span(id);
        let name = self.ctx.b.pool_string(l.name).to_owned();
        match self.lookup(&name) {
            None => self.report_at(REFERENCE_UNRESOLVED, sp, Arg::None),
            Some(Name::Record(idx)) => self.apply_target(p, idx)?,
            Some(_) => self.report_at(REFERENCE_AMBIGUOUS, sp, Arg::None),
        }
        Ok(())
    }

    fn apply_target(&mut self, p: u32, idx: u32) -> Result<(), Fail> {
        let Some(last) = self.resolve_record(idx)? else { return Ok(()) };
        self.ctx.b.resolve_link(p, last.node, last.kind, last.destination);
        let node = self.records[idx as usize].node;
        self.mark_referenced(node);
        Ok(())
    }

    /// Pairs anonymous references with anonymous targets in order.
    pub fn resolve_anonymous(&mut self) -> Result<(), Fail> {
        let (refs, targets) = (self.anon_refs.len(), self.anon_targets.len());
        if refs != targets {
            let at = match self.anon_refs.first() {
                Some(&id) => id,
                None => self.records[self.anon_targets[0] as usize].node,
            };
            let sp = self.node_span(at);
            self.ctx.report(ANONYMOUS_MISMATCH, sp, arg_int(refs as i64), arg_int(targets as i64));
            return Ok(());
        }
        for i in 0..refs {
            let Some((l, p)) = self.row(self.anon_refs[i]) else { continue };
            if l.kind == guidedoc::LinkKind::Unresolved {
                self.apply_target(p, self.anon_targets[i])?;
            }
        }
        Ok(())
    }
}
