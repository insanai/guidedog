//! Method resolution orders, class attributes along them, and `__slots__`.

use std::collections::VecDeque;
use std::sync::Arc;

use crate::literal::str_value;
use crate::resolve::{Object, ObjectKind, is_builtin_exception};
use crate::syntax::Node;
use crate::{ClassRef, DefKind, DefRef, Project};

/// Bounds how deep MRO computations nest: a class's MRO needs its bases' first. A
/// deeper inheritance chain gets an incomplete MRO.
pub(crate) const MAX_MRO_DEPTH: usize = 100;

const OBJECT: &str = "builtins.object";

/// One class of a method resolution order: a class in the roots, or one outside them
/// known by path.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MroEntry {
    /// The class, when it is in the roots.
    pub class: Option<ClassRef>,
    /// For a class outside the roots: `builtins.object`, `werkzeug.X`, ...
    pub path: String,
}

impl MroEntry {
    fn of_class(c: ClassRef) -> MroEntry {
        MroEntry { class: Some(c), path: String::new() }
    }

    fn of_path(path: &str) -> MroEntry {
        MroEntry { class: None, path: path.to_string() }
    }

    fn same(&self, other: &MroEntry) -> bool {
        self.class == other.class && (self.class.is_some() || self.path == other.path)
    }
}

/// A class's MRO as computed (or being computed), cached by the project.
#[derive(Debug, Clone)]
pub(crate) struct MroState {
    order: Arc<[MroEntry]>,
    complete: bool,
}

/// One entry of a class's `__slots__`, with the docstring a dictionary gives it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Slot {
    /// The slot's name.
    pub name: String,
    /// Its docstring, from a `__slots__` dictionary.
    pub doc: Option<String>,
}

impl Project {
    /// The class's method resolution order, by C3 linearisation, ending with `object`.
    /// The flag is false when a base could not be resolved or lies outside the roots (it
    /// then appears by path, and what it defines is unknown).
    pub fn mro(&mut self, c: ClassRef) -> (Arc<[MroEntry]>, bool) {
        if let Some(state) = self.mro_cache.get(&c) {
            return (Arc::clone(&state.order), state.complete);
        }
        // Recorded first: a class among its own bases ends the recursion.
        let empty = MroState { order: Arc::from(Vec::new()), complete: true };
        self.mro_cache.insert(c, empty);
        // A base's MRO comes first, so the computations nest as deep as the chain of
        // bases; MAX_MRO_DEPTH cuts it.
        self.bounds.mro_depth += 1;
        let mut complete = true;
        let depth = self.bounds.mro_depth;
        let order: Arc<[MroEntry]> =
            self.linearise(MroEntry::of_class(c), &mut complete, depth).into();
        self.bounds.mro_depth -= 1;
        self.mro_cache.insert(c, MroState { order: Arc::clone(&order), complete });
        (order, complete)
    }

    fn linearise(&mut self, e: MroEntry, complete: &mut bool, depth: usize) -> Vec<MroEntry> {
        let class = match e.class {
            Some(c) if depth <= MAX_MRO_DEPTH => c,
            _ => {
                if e.class.is_some() || e.path != OBJECT {
                    *complete = false;
                }
                let mut out = vec![e.clone()];
                if e.path != OBJECT {
                    out.push(MroEntry::of_path(OBJECT));
                }
                return out;
            }
        };
        let mut seqs: Vec<VecDeque<MroEntry>> = Vec::new();
        let mut heads: VecDeque<MroEntry> = VecDeque::new();
        for b in self.base_objects(class) {
            let entry = match (b.kind, b.class) {
                (ObjectKind::Class, Some(c)) => MroEntry::of_class(c),
                (ObjectKind::Builtin | ObjectKind::External, _) => MroEntry::of_path(&b.path),
                _ => {
                    *complete = false;
                    continue;
                }
            };
            if is_typing_generic(&entry.path) {
                continue; // Generic adds nothing documented.
            }
            seqs.push(self.base_order(&entry, complete));
            heads.push_back(entry);
        }
        let mut out = vec![e];
        if heads.is_empty() {
            out.push(MroEntry::of_path(OBJECT));
            return out;
        }
        seqs.push(heads);
        merge(&mut out, &mut seqs);
        out
    }

    /// A copy of a base's MRO, which `merge` consumes.
    fn base_order(&mut self, e: &MroEntry, complete: &mut bool) -> VecDeque<MroEntry> {
        let Some(c) = e.class else {
            let mut out = VecDeque::from([e.clone()]);
            if e.path != OBJECT {
                out.push_back(MroEntry::of_path(OBJECT));
                *complete = false;
            }
            return out;
        };
        let (order, whole) = self.mro(c);
        *complete &= whole;
        order.iter().cloned().collect()
    }

    /// Resolves the bases a class statement lists; subscripted generics give their
    /// origin.
    pub fn base_objects(&mut self, c: ClassRef) -> Vec<Object> {
        let module = Arc::clone(self.module(c.module));
        let class = module.class(c.class);
        let outer = class.outer.map(|class| ClassRef { module: c.module, class });
        class.bases.iter().map(|&b| self.reference(&module, outer, b)).collect()
    }

    /// `getattr(cls, name)`: the first class along the MRO whose body binds `name`.
    pub fn class_attr(&mut self, c: ClassRef, name: &str) -> Object {
        self.class_attr_at(c, name, 0)
    }

    pub(crate) fn class_attr_at(&mut self, c: ClassRef, name: &str, depth: usize) -> Object {
        let (order, _) = self.mro(c);
        // A class outside the roots may define anything; its members are unknown.
        for class in order.iter().filter_map(|e| e.class) {
            let scope = &self.class(class).scope;
            let Some(i) = scope.find_def_index(name) else { continue };
            if scope.defs[i].kind == DefKind::Annotation {
                continue;
            }
            let d = DefRef { module: class.module, class: Some(class.class), index: i as u32 };
            return self.def_object_at(d, depth + 1);
        }
        Object::default()
    }

    /// `inspect`'s `getslots`: the `__slots__` the class has (its own, or the nearest
    /// base's), as a list, tuple, dictionary, or single string of names. `None` when
    /// there are none, or they are computed.
    pub fn slots(&mut self, c: ClassRef) -> Option<Vec<Slot>> {
        let (order, _) = self.mro(c);
        for class in order.iter().filter_map(|e| e.class) {
            let module = self.module(class.module);
            let Some(d) = module.class(class.class).scope.find_def("__slots__") else { continue };
            if d.kind != DefKind::Assign || d.tuple {
                continue;
            }
            return slot_list(module.node(d.value));
        }
        None
    }

    /// Whether a class derives from `BaseException`, as far as its MRO is known.
    pub fn is_exception(&mut self, c: ClassRef) -> bool {
        let (order, _) = self.mro(c);
        order.iter().filter(|e| e.class.is_none()).any(|e| {
            let builtin = e.path.strip_prefix("builtins.").is_some_and(is_builtin_exception);
            // Common exception bases outside the roots.
            builtin || ["Error", "Exception", "Warning"].iter().any(|s| e.path.ends_with(s))
        })
    }
}

fn slot_list(value: Node<'_>) -> Option<Vec<Slot>> {
    if let Some(name) = str_value(value) {
        return Some(vec![Slot { name: name.into_owned(), doc: None }]);
    }
    let mut out = Vec::new();
    match value.kind() {
        "list" | "tuple" => {
            for item in value.named_children().filter(|i| i.kind() != "comment") {
                out.push(Slot { name: str_value(item)?.into_owned(), doc: None });
            }
        }
        "dictionary" => {
            for item in value.named_children().filter(|i| i.kind() == "pair") {
                let name = str_value(item.field("key"))?.into_owned();
                let doc = str_value(item.field("value")).map(|d| d.into_owned());
                out.push(Slot { name, doc });
            }
        }
        _ => return None,
    }
    Some(out)
}

fn is_typing_generic(path: &str) -> bool {
    matches!(
        path,
        "typing.Generic"
            | "typing_extensions.Generic"
            | "typing.Protocol"
            | "typing_extensions.Protocol"
    )
}

/// C3's merge; an inconsistent order falls back to taking heads in turn.
fn merge(out: &mut Vec<MroEntry>, seqs: &mut [VecDeque<MroEntry>]) {
    loop {
        let heads = seqs.iter().filter_map(|s| s.front());
        let mut candidate = None;
        let mut any = false;
        for head in heads {
            any = true;
            if !in_tails(head, seqs) {
                candidate = Some(head.clone());
                break;
            }
        }
        if !any {
            return;
        }
        let candidate = match candidate {
            Some(c) => c,
            None => match seqs.iter().find_map(|s| s.front()) {
                Some(first) => first.clone(),
                None => return,
            },
        };
        for s in seqs.iter_mut() {
            if s.front().is_some_and(|f| f.same(&candidate)) {
                s.pop_front();
            }
        }
        out.push(candidate);
    }
}

fn in_tails(e: &MroEntry, seqs: &[VecDeque<MroEntry>]) -> bool {
    seqs.iter().any(|s| s.iter().skip(1).any(|x| x.same(e)))
}
