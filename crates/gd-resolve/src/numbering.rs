//! Section and figure numbers (numbering.odin).
//!
//! Section numbers follow Sphinx's `assign_section_numbers`. Each `:numbered:` toctree
//! starts a numbering at 1. The documents it lists are numbered in turn, and within a
//! document its sections and the documents of the toctrees standing among them are
//! siblings in document order: a toctree before a section's first subsection numbers
//! its documents first. A numbered toctree's depth counts the levels numbered below
//! it. A document is numbered once: a toctree that lists a document already numbered
//! warns, as a numbered toctree nested in another does. Documents are taken in reading
//! order, then the ones no toctree reaches.

use std::collections::HashMap;

use gd_env::{DocId, LabelKind};

use crate::tables::{Pass, doc_warning};

/// How deep documents may nest through toctrees while numbering: deeper ones keep no
/// number rather than exhaust the stack.
const MAX_NESTING: usize = 256;

struct SectionNumbering<'p, 'a> {
    pass: &'p mut Pass<'a>,
    /// Per document: numbered, or holding a numbered toctree.
    assigned: Vec<bool>,
    nesting: usize,
}

pub(crate) fn number_sections(pass: &mut Pass<'_>) {
    let n = pass.env.docs.len();
    let order = pass.tables.order.clone();
    let mut seen = vec![false; n];
    let mut s = SectionNumbering { pass, assigned: vec![false; n], nesting: 0 };
    for id in order {
        seen[id.index()] = true;
        s.number_toctrees_of(id);
    }
    for (i, _) in seen.iter().enumerate().filter(|(_, seen)| !**seen) {
        s.number_toctrees_of(DocId(i as u32));
    }
}

impl SectionNumbering<'_, '_> {
    /// Applies the numbered toctrees a document holds.
    fn number_toctrees_of(&mut self, id: DocId) {
        let env = self.pass.env;
        let d = env.doc(id);
        if !d.toctrees.iter().any(|t| t.numbered != 0) {
            return;
        }
        self.assigned[id.index()] = true;
        for (ti, tree) in d.toctrees.iter().enumerate() {
            if tree.numbered != 0 {
                let mut count = 0;
                self.number_toctree(id, ti, &[], &mut count, tree.numbered as i64);
            }
        }
    }

    /// Numbers the documents a toctree lists as siblings at one level, continuing
    /// `count` below `prefix`, `depth` levels deep.
    fn number_toctree(
        &mut self,
        id: DocId,
        tree: usize,
        prefix: &[u32],
        count: &mut u32,
        depth: i64,
    ) {
        let entries = match self.pass.tables.trees[id.index()].get(tree) {
            Some(entries) if depth > 0 && self.nesting < MAX_NESTING => entries.clone(),
            _ => return,
        };
        for e in entries {
            // A URL, a generated page, or self.
            let Some(target) = e.doc.filter(|&t| t != id) else { continue };
            if self.assigned[target.index()] {
                self.warn_renumbered(id, tree, target);
                continue;
            }
            self.assigned[target.index()] = true;
            self.nesting += 1;
            self.number_level(target, None, prefix, count, depth);
            self.nesting -= 1;
        }
    }

    fn warn_renumbered(&mut self, id: DocId, tree: usize, target: DocId) {
        let env = self.pass.env;
        let d = env.doc(id);
        let message = format!(
            "{} already has section numbers from another numbered toctree, which it keeps.",
            env.doc(target).docname
        );
        let line = d.toctrees.get(tree).map_or(0, |t| t.line);
        self.pass.reports.push(doc_warning(
            d,
            line,
            [
                "build.toctree.renumbered",
                "DOCUMENT NUMBERED TWICE",
                &message,
                "List it in one numbered toctree only, or remove :numbered: from a toctree \
             nested in a numbered one.",
                "toc.secnum",
            ],
        ));
    }

    /// Numbers the sections of a document whose parent is `parent` (`None`: its top
    /// level) and the documents of the toctrees standing directly in it, in document
    /// order, continuing `count` below `prefix`.
    fn number_level(
        &mut self,
        id: DocId,
        parent: Option<u32>,
        prefix: &[u32],
        count: &mut u32,
        depth: i64,
    ) {
        let env = self.pass.env;
        let d = env.doc(id);
        let mut tree = 0; // the next toctree of the document to consider.
        for (i, s) in d.sections.iter().enumerate() {
            if s.parent != parent || s.in_object {
                continue;
            }
            tree = self.number_trees_before(id, parent, i, tree, prefix, count, depth);
            *count += 1;
            let mut number = prefix.to_vec();
            number.push(*count);
            if depth > 0 {
                self.pass.tables.secnumbers[id.index()][i] = join_numbers(&number);
            }
            let mut below = 0;
            self.number_level(id, Some(i as u32), &number, &mut below, depth - 1);
        }
        self.number_trees_before(id, parent, d.sections.len(), tree, prefix, count, depth);
    }

    /// Numbers the documents of the toctrees in section `parent` that come before the
    /// section at index `before`, from toctree `from`; returns the next one.
    #[allow(clippy::too_many_arguments)] // the numbering state of one level.
    fn number_trees_before(
        &mut self,
        id: DocId,
        parent: Option<u32>,
        before: usize,
        from: usize,
        prefix: &[u32],
        count: &mut u32,
        depth: i64,
    ) -> usize {
        let trees = &self.pass.env.doc(id).toctrees;
        let mut t = from;
        while t < trees.len() && trees[t].position as usize <= before {
            if trees[t].section == parent {
                self.number_toctree(id, t, prefix, count, depth);
            }
            t += 1;
        }
        t
    }
}

fn join_numbers(numbers: &[u32]) -> String {
    numbers.iter().map(u32::to_string).collect::<Vec<_>>().join(".")
}

/// Per kind of item, per chapter prefix: the items counted so far.
type Counters = HashMap<(LabelKind, String), u32>;

/// Numbers figures, tables, captioned code and equations in reading order: the
/// documents a toctree lists are counted where the toctree stands, before what follows
/// it. With section numbers, numbers restart per section at `numfig_secnum_depth` and
/// carry its prefix (`2.3`); otherwise they count across the project.
pub(crate) fn number_figures(pass: &mut Pass<'_>) {
    let mut counters = Counters::new();
    let mut done = vec![false; pass.env.docs.len()];
    for id in pass.tables.order.clone() {
        number_document(pass, id, &mut counters, &mut done, 0);
    }
}

fn number_document(
    pass: &mut Pass<'_>,
    id: DocId,
    counters: &mut Counters,
    done: &mut [bool],
    depth: usize,
) {
    if done[id.index()] || depth > 64 {
        return;
    }
    done[id.index()] = true;
    let c = pass.config;
    let env = pass.env;
    let d = env.doc(id);
    let tree_count = pass.tables.trees[id.index()].len();
    let (mut equations, mut trees) = (0, 0);
    for (i, item) in d.numbered.iter().enumerate() {
        while trees < (item.toctrees as usize).min(tree_count) {
            number_entries(pass, id, trees, counters, done, depth);
            trees += 1;
        }
        if item.kind == LabelKind::Equation && !(c.numfig && c.math_numfig) {
            equations += 1;
            pass.tables.fignumbers[id.index()][i] = equations.to_string();
            continue;
        }
        if !c.numfig {
            continue;
        }
        let prefix = chapter_prefix(pass, id, item.section, c.numfig_secnum_depth);
        let count = counters.entry((item.kind, prefix.clone())).or_insert(0);
        *count += 1;
        let number =
            if prefix.is_empty() { count.to_string() } else { format!("{prefix}.{count}") };
        pass.tables.fignumbers[id.index()][i] = number;
    }
    while trees < tree_count {
        number_entries(pass, id, trees, counters, done, depth);
        trees += 1;
    }
}

fn number_entries(
    pass: &mut Pass<'_>,
    id: DocId,
    tree: usize,
    counters: &mut Counters,
    done: &mut [bool],
    depth: usize,
) {
    let targets: Vec<DocId> =
        pass.tables.trees[id.index()][tree].iter().filter_map(|e| e.doc).collect();
    for target in targets {
        number_document(pass, target, counters, done, depth + 1);
    }
}

/// The section number figures in `section` carry: the first `depth` parts of the
/// section's number, or of the document title's when the section has none.
fn chapter_prefix(pass: &Pass<'_>, id: DocId, section: Option<u32>, depth: i64) -> String {
    if depth <= 0 {
        return String::new();
    }
    let numbers = &pass.tables.secnumbers[id.index()];
    let own = section.and_then(|s| numbers.get(s as usize)).filter(|n| !n.is_empty());
    let Some(number) = own.or_else(|| numbers.first().filter(|n| !n.is_empty())) else {
        return String::new();
    };
    number.split('.').take(depth as usize).collect::<Vec<_>>().join(".")
}
