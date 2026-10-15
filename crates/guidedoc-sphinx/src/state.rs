//! The reader state Sphinx keeps per document (its `env.ref_context` and `temp_data`):
//! the current domain, module, classes, program, highlighting, and serial numbers.

use std::collections::HashSet;

use crate::config::Config;
use crate::domains::{odin::OdinState, rust::RustState};

/// The deepest nesting of context names (classes, objects, scopes) kept.
pub(crate) const MAX_NEST: usize = 32;

/// A stack of nested context names (classes, objects, scopes), at most `MAX_NEST` deep.
#[derive(Debug, Clone, Default)]
pub(crate) struct NameStack {
    items: Vec<String>,
}

impl NameStack {
    /// Pushes `name`; false (and nothing pushed) when the stack is full.
    pub(crate) fn push(&mut self, name: &str) -> bool {
        if self.items.len() >= MAX_NEST {
            return false;
        }
        self.items.push(name.to_owned());
        true
    }

    /// Removes the top name, if any.
    pub(crate) fn pop(&mut self) {
        self.items.pop();
    }

    /// Returns the top name, or "" for an empty stack.
    pub(crate) fn top(&self) -> &str {
        self.items.last().map_or("", String::as_str)
    }

    /// Returns the names, outermost first.
    pub(crate) fn items(&self) -> &[String] {
        &self.items
    }

    /// Returns the number of names.
    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    /// Removes every name.
    pub(crate) fn clear(&mut self) {
        self.items.clear();
    }
}

/// A C++ scope saved while an object's content is read: the symbol path, its object
/// type, the directive that declared it, and whether it is a concept.
#[derive(Debug, Clone, Default)]
pub(crate) struct CppScope {
    pub path: String,
    pub objtype: String,
    pub directive: String,
    pub in_concept: bool,
}

/// The per-document state of the Sphinx layer. Domains keep their context here; the
/// Odin and Rust domains keep theirs in `odin` and `rust`.
#[derive(Debug, Clone, Default)]
pub(crate) struct State {
    /// The default domain; "" for none.
    pub domain: String,
    /// The highlighting language.
    pub highlight: String,
    /// The highlight directive's linenothreshold; `None` for none.
    pub linenothreshold: Option<i64>,
    pub py_module: String,
    pub py_classes: NameStack,
    pub py_class: String,
    /// Modules saved by a `:module:` option.
    pub py_modules: NameStack,
    /// The names the current Python object's signatures gave (prefix, name).
    pub py_names: Vec<(String, String)>,
    pub js_module: String,
    pub js_objects: NameStack,
    pub c_scope: NameStack,
    pub c_namespace: String,
    /// Saved by c:namespace-push.
    pub c_namespaces: NameStack,
    pub cpp_scope: NameStack,
    /// The C++ objects content is nested in (cpp_domain_name).
    pub cpp_objects: NameStack,
    pub cpp_namespace: String,
    /// Saved by cpp:namespace-push.
    pub cpp_namespaces: NameStack,
    /// The path of the symbol C++ declarations are declared in.
    pub cpp_scope_path: String,
    /// Its object type ("" for a namespace or the root).
    pub cpp_parent_objtype: String,
    /// The directive that declared it.
    pub cpp_parent_directive: String,
    /// Inside a concept's content (no index entries).
    pub cpp_in_concept: bool,
    /// Saved C++ scopes.
    pub cpp_scopes: Vec<CppScope>,
    /// Symbols cpp:namespace-push opened.
    pub cpp_namespace_paths: NameStack,
    /// The current program (std:program).
    pub program: String,
    /// The rst:directive objects content is nested in.
    pub rst_directives: NameStack,
    /// The id the last object signature got: its entries' target.
    pub object_id: String,
    /// Reading the role inside an :external: role.
    pub external: bool,
    /// The inventory an :external+name: role names.
    pub inventory: String,
    pub index_serial: u32,
    pub equation_serial: u32,
    /// Serial numbers of ids, by prefix.
    pub serials: Vec<(String, u32)>,
    /// Every id given in this document.
    pub ids: HashSet<String>,
    /// How many generated autodoc sources are being parsed.
    pub autodoc_depth: usize,
    /// The Odin domain's state.
    pub odin: OdinState,
    /// The Rust domain's state.
    pub rust: RustState,
}

impl State {
    /// The state a document read starts with.
    pub(crate) fn new(config: &Config) -> State {
        State {
            domain: config.primary_domain.clone(),
            highlight: config.highlight_language.clone(),
            ..State::default()
        }
    }
}
