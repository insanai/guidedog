//! Deciding what is documented where, before any text is written: which modules get a
//! page, which items each page describes, and the path every described item is known by.
//!
//! Rustdoc's rules are followed: an item is documented in the module that declares it
//! when every module on the way is public; a `pub use` of such an item is listed as a
//! re-export; a `pub use` of an item that is not reachable otherwise (it lives in a
//! private module) documents the item in place, under the name the re-export gives it.

use crate::Options;
use crate::model::{Crate, Item, ItemId, ItemKind, ModuleId};
use crate::resolve::{Namespace, Target, follow_use, resolve_path};
use std::collections::{HashMap, VecDeque};

/// One entry of a module page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Entry {
    /// An item described on this page, under `name`.
    Item { id: ItemId, name: String },
    /// A child module, which has its own page.
    Module { id: ModuleId, name: String },
    /// A re-export listed as a `use` line.
    Use { id: ItemId },
}

/// The page of one module.
#[derive(Debug, Clone)]
pub(crate) struct PlanPage {
    pub module: ModuleId,
    /// The Rust path the page documents the module at: `my_crate::parser`.
    pub path: String,
    pub entries: Vec<Entry>,
}

/// The whole plan: pages in the order modules were reached, and documented paths.
#[derive(Debug, Default)]
pub(crate) struct Plan {
    pub pages: Vec<PlanPage>,
    item_paths: HashMap<ItemId, String>,
    module_paths: HashMap<ModuleId, String>,
    /// Which type each attached impl block belongs to.
    impl_owner: HashMap<ItemId, ItemId>,
}

/// Plans the pages of `krate`.
pub(crate) fn plan(krate: &Crate, options: &Options) -> Plan {
    let mut planner = Planner { krate, options, plan: Plan::default(), queue: VecDeque::new() };
    for (index, item) in krate.items.iter().enumerate() {
        for &block in &item.impls {
            planner.plan.impl_owner.insert(block, ItemId(index as u32));
        }
    }
    planner.queue.push_back((ModuleId(0), krate.name.clone()));
    while let Some((module, path)) = planner.queue.pop_front() {
        if planner.plan.module_paths.contains_key(&module) {
            continue;
        }
        planner.plan.module_paths.insert(module, path.clone());
        let entries = planner.entries(module, &path);
        planner.plan.pages.push(PlanPage { module, path, entries });
    }
    planner.plan
}

impl Plan {
    /// The path a module is documented at, if it is documented.
    pub fn module_path(&self, module: ModuleId) -> Option<&str> {
        self.module_paths.get(&module).map(String::as_str)
    }

    /// The path an item is documented at: a top-level item's own, or its owner's path
    /// and its name for a member (methods of impls go under the impl's type). `None`
    /// when it is not documented.
    pub fn item_path(&self, krate: &Crate, id: ItemId) -> Option<String> {
        if let Some(path) = self.item_paths.get(&id) {
            return Some(path.clone());
        }
        let item = krate.item(id);
        let parent = item.parent?;
        let owner = match krate.item(parent).kind {
            ItemKind::Impl => *self.impl_owner.get(&parent)?,
            _ => parent,
        };
        Some(format!("{}::{}", self.item_path(krate, owner)?, item.name))
    }

    /// The documented path of a resolved target.
    pub fn target_path(&self, krate: &Crate, target: Target) -> Option<String> {
        match target {
            Target::Module(m) => self.module_path(m).map(str::to_string),
            Target::Item(id) => self.item_path(krate, id),
        }
    }
}

/// Whether an item is documented at all with these options, regardless of where.
pub(crate) fn is_shown(item: &Item, options: &Options) -> bool {
    if item.is_test || (item.is_hidden && !options.document_hidden) {
        return false;
    }
    options.include_private || item.visibility.is_public()
}

struct Planner<'k> {
    krate: &'k Crate,
    options: &'k Options,
    plan: Plan,
    queue: VecDeque<(ModuleId, String)>,
}

impl Planner<'_> {
    /// Whether a module is documented where it is declared: every `mod` on the way from
    /// the root is shown.
    fn is_natural_module(&self, module: ModuleId) -> bool {
        let mut current = module;
        while let Some(item) = self.krate.module(current).item {
            if !is_shown(self.krate.item(item), self.options) {
                return false;
            }
            match self.krate.module(current).parent {
                Some(parent) => current = parent,
                None => break,
            }
        }
        true
    }

    /// Whether a top-level item is documented where it is declared.
    fn is_natural_item(&self, id: ItemId) -> bool {
        let item = self.krate.item(id);
        let exported = item.kind == ItemKind::Macro && item.qualifiers.is_exported;
        is_shown(item, self.options) && (exported || self.is_natural_module(item.module))
    }

    /// The entries of a module's page, documented at `path`.
    fn entries(&mut self, module: ModuleId, path: &str) -> Vec<Entry> {
        let mut entries = Vec::new();
        for &id in &self.krate.module(module).items {
            let item = self.krate.item(id);
            match item.kind {
                ItemKind::Use => self.add_use(id, path, &mut entries),
                ItemKind::Module => self.add_module(id, path, &mut entries),
                ItemKind::Impl => {
                    if !self.plan.impl_owner.contains_key(&id) && !item.is_test {
                        entries.push(Entry::Item { id, name: String::new() });
                    }
                }
                // Exported macros live at the crate root, added below.
                ItemKind::Macro if item.qualifiers.is_exported => {}
                _ if is_shown(item, self.options) => self.add_item(id, path, &mut entries),
                _ => {}
            }
        }
        if module == ModuleId(0) {
            self.add_exported_macros(&mut entries);
        }
        entries
    }

    fn add_item(&mut self, id: ItemId, path: &str, entries: &mut Vec<Entry>) {
        let name = self.krate.item(id).name.clone();
        if self.plan.item_paths.contains_key(&id) {
            return;
        }
        self.plan.item_paths.insert(id, format!("{path}::{name}"));
        entries.push(Entry::Item { id, name });
    }

    fn add_exported_macros(&mut self, entries: &mut Vec<Entry>) {
        for (index, item) in self.krate.items.iter().enumerate() {
            let exported = item.kind == ItemKind::Macro && item.qualifiers.is_exported;
            if exported && is_shown(item, self.options) {
                let path = self.krate.name.clone();
                self.add_item(ItemId(index as u32), &path, entries);
            }
        }
    }

    fn add_module(&mut self, id: ItemId, path: &str, entries: &mut Vec<Entry>) {
        let item = self.krate.item(id);
        let Some(target) = item.module_target else { return };
        if !is_shown(item, self.options) {
            return;
        }
        self.queue.push_back((target, format!("{path}::{}", item.name)));
        entries.push(Entry::Module { id: target, name: item.name.clone() });
    }

    /// A `pub use`: a re-export line, or the item documented in place when it is not
    /// reachable otherwise (or `#[doc(inline)]` asks for it).
    fn add_use(&mut self, id: ItemId, path: &str, entries: &mut Vec<Entry>) {
        let item = self.krate.item(id);
        if !is_shown(item, self.options) || !item.visibility.is_public() || item.name == "_" {
            return;
        }
        if item.name == "*" {
            self.add_glob(id, path, entries);
            return;
        }
        let name = item.name.clone();
        let wants_inline = item.inline == Some(true);
        let refuses_inline = item.inline == Some(false);
        match follow_use(self.krate, id, Namespace::Any, crate::resolve::MAX_HOPS) {
            Some(Target::Item(target)) => {
                let natural = self.is_natural_item(target);
                let taken = self.plan.item_paths.contains_key(&target);
                if (natural && !wants_inline)
                    || taken
                    || refuses_inline
                    || self.krate.item(target).is_test
                {
                    entries.push(Entry::Use { id });
                } else {
                    self.plan.item_paths.insert(target, format!("{path}::{name}"));
                    entries.push(Entry::Item { id: target, name });
                }
            }
            Some(Target::Module(target)) => {
                let natural = self.is_natural_module(target);
                if natural || refuses_inline || self.plan.module_paths.contains_key(&target) {
                    entries.push(Entry::Use { id });
                } else {
                    self.queue.push_back((target, format!("{path}::{name}")));
                    entries.push(Entry::Module { id: target, name });
                }
            }
            None => entries.push(Entry::Use { id }),
        }
    }

    /// `pub use path::*`: a re-export line when the module is documented anyway,
    /// otherwise each of its shown items documented here.
    fn add_glob(&mut self, id: ItemId, path: &str, entries: &mut Vec<Entry>) {
        let item = self.krate.item(id);
        let base = item.use_path.trim_end_matches("::*");
        let source = resolve_path(self.krate, item.module, None, base, Namespace::Any);
        let Some(Target::Module(source)) = source else {
            entries.push(Entry::Use { id });
            return;
        };
        if self.is_natural_module(source) || item.inline == Some(false) {
            entries.push(Entry::Use { id });
            return;
        }
        for &child in &self.krate.module(source).items {
            let inner = self.krate.item(child);
            let skip = matches!(inner.kind, ItemKind::Use | ItemKind::Impl)
                || !is_shown(inner, self.options)
                || !inner.visibility.is_public();
            if skip {
                continue;
            }
            if inner.kind == ItemKind::Module {
                self.add_module(child, path, entries);
            } else {
                self.add_item(child, path, entries);
            }
        }
    }
}
