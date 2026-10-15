//! Substitution references match definitions case-sensitively first, then
//! case-insensitively. A later definition of the same name replaces the earlier one and
//! is reported.

use guidedoc::{Arg, Fail, LinkKind, NONE, NodeId, NodeKind, Text, arg_int, arg_span};

use super::{Definition, Resolver};
use crate::messages::{
    SUBSTITUTION_CYCLE, SUBSTITUTION_DEPTH, SUBSTITUTION_DUPLICATE, SUBSTITUTION_UNDEFINED,
};
use crate::roles::fold_key;

impl Resolver<'_, '_> {
    pub fn add_definition(&mut self, id: NodeId) {
        self.defs.push(Definition { node: id, first_edge: 0, edge_count: 0, color: 0 });
        let name = self.text_of(id).to_owned();
        if name.is_empty() {
            return;
        }
        let folded = fold_key(&name);
        if let Some(old) = self.subs.insert(name, id) {
            let (a, b) = (self.quote_span(id), self.quote_span(old));
            self.ctx.report(SUBSTITUTION_DUPLICATE, a, arg_span(a), arg_span(b));
        }
        self.subs_folded.insert(folded, id);
    }

    /// Links a reference to its definition and returns the definition's index.
    pub fn resolve_substitution(&mut self, id: NodeId) -> Result<Option<usize>, Fail> {
        let name = self.text_of(id).to_owned();
        let mut def = self.subs.get(&name).copied();
        if def.is_none() && self.default_substitution(id, &name)? {
            return Ok(None);
        }
        if def.is_none() {
            def = self.subs_folded.get(&fold_key(&name)).copied();
        }
        let Some(def) = def else {
            let sp = self.node_span(id);
            self.report_at(SUBSTITUTION_UNDEFINED, sp, Arg::None);
            return Ok(None);
        };
        let row = self.ensure_row(id)?;
        self.ctx.b.resolve_link(row, def, LinkKind::Internal, Text::default());
        Ok(self.defs.binary_search_by_key(&def, |d| d.node).ok())
    }

    /// Finds definitions that use themselves, directly or through other definitions,
    /// with an explicit depth-first search bounded by the number of definitions and by
    /// `limits.max_expansion`.
    pub fn check_substitutions(&mut self) -> Result<(), Fail> {
        let limit = self.ctx.limits.max_expansion.max(1);
        let mut frames: Vec<(usize, usize)> = Vec::new();
        for start in 0..self.defs.len() {
            if self.defs[start].color != 0 {
                continue;
            }
            frames.clear();
            frames.push((start, 0));
            self.defs[start].color = 1;
            while let Some(frame) = frames.last_mut() {
                self.ctx.b.spend(1)?;
                let def = self.defs[frame.0];
                if frame.1 >= def.edge_count {
                    self.defs[frame.0].color = 2;
                    frames.pop();
                    continue;
                }
                let edge = self.edges[def.first_edge + frame.1];
                frame.1 += 1;
                let sp = self.node_span(edge.at);
                match self.defs[edge.to].color {
                    1 => self.report_at(SUBSTITUTION_CYCLE, sp, Arg::None),
                    0 if frames.len() >= limit => {
                        self.report_at(SUBSTITUTION_DEPTH, sp, arg_int(limit as i64));
                    }
                    0 => {
                        self.defs[edge.to].color = 1;
                        frames.push((edge.to, 0));
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Replaces a reference with the dialect's default text for its name, when it has
    /// one, as Sphinx's DefaultSubstitutions transform does before Docutils'
    /// Substitutions: the reference becomes text.
    fn default_substitution(&mut self, id: NodeId, name: &str) -> Result<bool, Fail> {
        let Some(text) = self.substitute.and_then(|f| f(name)) else { return Ok(false) };
        self.ctx.b.set_kind(id, NodeKind::Text);
        let t = self.ctx.b.add_text(&text)?;
        self.ctx.b.set_text(id, t);
        self.ctx.b.snap.nodes[id.index()].payload = NONE; // text has no link row.
        Ok(true)
    }
}
