//! A C++ symbol path: the symbols from the root to a declared name, as Sphinx's symbol
//! tree has them. Each element is a symbol's identifier or operator, its template
//! arguments (when they specialize) and template parameters, as `str()` gives them, its
//! display text, the element's id in each id version, and the newest id of the symbol's
//! declaration when it declares one.
//!
//! Readers keep paths as text in node attributes (`cpp:path`, `cpp:parent`): one element
//! per line, fields apart by `FIELD`, a missing id as `NO_ID`. `encode_path` and
//! `decode_path` convert, so the host can build the project's symbol tree from them.

use crate::ast::{Ast, NodeId};
use crate::ids::{Ids, MAX_ID_VERSION};

/// Separates the fields of an encoded path element.
pub const FIELD: char = '\x1f';

/// Stands for an element's id in a version that has none.
pub const NO_ID: &str = "\x1e";

/// One symbol of a path.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Element {
    /// The identifier or operator, as `str()` gives it.
    pub ident: String,
    /// The template arguments when they specialize, or `""`.
    pub args: String,
    /// The template parameter list the symbol takes, or `""`.
    pub params: String,
    /// The display text: the identifier (`[anonymous]` for `@...`) and arguments.
    pub display: String,
    /// The element's id in versions 1 to 4 (index 0 is version 1); `None` where the
    /// version has none.
    pub ids: [Option<String>; MAX_ID_VERSION],
    /// The newest id of the symbol's declaration, or `""` when it declares none.
    pub decl: String,
}

impl Element {
    /// Returns the element's id in `version` (1 to 4).
    pub fn id(&self, version: usize) -> Option<&str> {
        self.ids.get(version.checked_sub(1)?)?.as_deref()
    }
}

/// Counts an encoded path's elements.
pub fn path_count(path: &str) -> usize {
    if path.is_empty() { 0 } else { path.bytes().filter(|&b| b == b'\n').count() + 1 }
}

/// Decodes a path's elements (empty for the root, `""`).
pub fn decode_path(path: &str) -> Vec<Element> {
    if path.is_empty() {
        return Vec::new();
    }
    path.split('\n').map(decode_element).collect()
}

fn decode_element(line: &str) -> Element {
    let mut fields = line.splitn(9, FIELD);
    let mut next = || fields.next().unwrap_or("").to_string();
    let (ident, args, params, display) = (next(), next(), next(), next());
    let ids = [(); MAX_ID_VERSION].map(|()| {
        let id = next();
        (id != NO_ID).then_some(id)
    });
    Element { ident, args, params, display, ids, decl: next() }
}

/// Appends the elements as an encoded path to `out`.
pub fn encode_path(elements: &[Element], out: &mut String) {
    for (i, e) in elements.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        for (k, field) in [&e.ident, &e.args, &e.params, &e.display].into_iter().enumerate() {
            if k > 0 {
                out.push(FIELD);
            }
            out.push_str(field);
        }
        for id in &e.ids {
            out.push(FIELD);
            out.push_str(id.as_deref().unwrap_or(NO_ID));
        }
        out.push(FIELD);
        out.push_str(&e.decl);
    }
}

/// Returns `str()` of the full nested name, from the root (`ns::C<T>::f`).
pub fn full_name(elements: &[Element]) -> String {
    let mut out = String::new();
    for (i, e) in elements.iter().enumerate() {
        if i > 0 {
            out.push_str("::");
        }
        out.push_str(&e.ident);
        out.push_str(&e.args);
    }
    out
}

/// Returns the display string of the full nested name, from the root.
pub fn display_name(elements: &[Element]) -> String {
    let mut out = String::new();
    for (i, e) in elements.iter().enumerate() {
        if i > 0 {
            out.push_str("::");
        }
        out.push_str(&e.display);
    }
    out
}

impl Ast<'_> {
    /// Returns the path elements of a `NestedName` as symbols get them from its elements
    /// (`_add_symbols` with strict template parameter lists): each element with template
    /// arguments takes the next parameter list of `lists`, the last the remaining one,
    /// and arguments that merely repeat the parameters are dropped.
    pub fn name_elements(&self, name: NodeId, lists: Option<NodeId>) -> Vec<Element> {
        let crate::ast::Node::NestedName { elements, .. } = *self.node(name) else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(self.count(elements));
        let mut k = 0;
        for e in self.list(elements) {
            let crate::ast::Node::NameElement { name, args, .. } = *self.node(e) else { continue };
            let params = if self.next(e).is_some() {
                let taken = args.and_then(|_| self.nth(lists, k));
                k += usize::from(args.is_some());
                taken
            } else {
                self.nth(lists, k)
            };
            let args = match (args, params) {
                (Some(a), Some(p)) if !self.is_specialization(p, a) => None,
                _ => args,
            };
            out.push(self.element(name, args, params));
        }
        out
    }

    /// Makes the path element of an identifier or operator with its template arguments
    /// and parameters.
    pub fn element(&self, name: NodeId, args: Option<NodeId>, params: Option<NodeId>) -> Element {
        let mut e = Element {
            ident: self.text(name, false),
            params: params.map(|p| self.text(p, false)).unwrap_or_default(),
            ..Element::default()
        };
        if let Some(args) = args {
            e.args = self.text(args, false);
        }
        self.write_text(name, true, &mut e.display);
        self.write_opt(args, true, &mut e.display);
        for v in 1..=MAX_ID_VERSION {
            let ids = Ids { ast: self, version: v, elements: &[], placeholder: false };
            let mut id = String::new();
            let ok = ids.id(name, &mut id).and_then(|()| match args {
                Some(args) => ids.id(args, &mut id),
                None => Some(()),
            });
            e.ids[v - 1] = ok.map(|()| id);
        }
        e
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_round_trip() {
        let e = Element {
            ident: "C".into(),
            args: "<int>".into(),
            params: "".into(),
            display: "C<int>".into(),
            ids: [None, Some("1CIiE".into()), Some("1CIiE".into()), Some("1CIiE".into())],
            decl: "_CPPv4".into(),
        };
        let mut text = String::new();
        encode_path(&[e.clone(), e.clone()], &mut text);
        assert_eq!(path_count(&text), 2);
        assert_eq!(decode_path(&text), vec![e.clone(), e]);
        assert!(decode_path("").is_empty());
    }
}
