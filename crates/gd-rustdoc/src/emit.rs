//! Writing the planned pages as reStructuredText of the `rust` domain.
//!
//! A module page is a title, the module's `rust:crate` or `rust:module` directive, the
//! module docs, then its items in rustdoc's groups (re-exports, modules, macros,
//! structs, ...), each group under a rubric. A type's directive holds its fields or
//! variants and the impl blocks attached to it, each with its associated items.

use crate::diag::Diagnostic;
use crate::inline::{Link, LinkResolver};
use crate::markdown::{plain_paragraph, summary, write_rst};
use crate::model::{Crate, FileId, Item, ItemId, ItemKind, ModuleId, Visibility};
use crate::plan::{Entry, Plan, PlanPage, is_shown};
use crate::resolve::{Namespace, Target, resolve_path};
use crate::{MemberOrder, Options, Page};

/// The indentation of directive content.
const BODY: &str = "   ";

/// Writes every planned page: `index.rst` first, then one page per module.
pub(crate) fn pages(krate: &Crate, plan: &Plan, options: &Options) -> Vec<Page> {
    let mut pages = Vec::with_capacity(plan.pages.len() + 1);
    pages.push(index_page(krate, plan));
    for page in &plan.pages {
        let mut writer = Writer { krate, plan, options, out: String::new(), warnings: Vec::new() };
        writer.module_page(page);
        pages.push(Page {
            path: page_path(&page.path),
            module: page.path.clone(),
            text: writer.out,
            warnings: writer.warnings,
        });
    }
    pages
}

/// The page path of a module path: `a::b::c` is `a/b/c.rst`.
fn page_path(module_path: &str) -> String {
    format!("{}.rst", module_path.replace("::", "/"))
}

fn write_title(title: &str, out: &mut String) {
    let title = plain_paragraph(title);
    out.push_str(&title);
    out.push('\n');
    out.push_str(&"=".repeat(title.chars().count().max(3)));
    out.push_str("\n\n");
}

/// The index: every module page in a hidden toctree, and a list of them with their
/// synopses.
fn index_page(krate: &Crate, plan: &Plan) -> Page {
    let mut out = String::new();
    write_title(&format!("{} API reference", krate.name), &mut out);
    out.push_str(".. toctree::\n   :hidden:\n\n");
    for page in &plan.pages {
        out.push_str(BODY);
        out.push_str(page_path(&page.path).trim_end_matches(".rst"));
        out.push('\n');
    }
    out.push('\n');
    for page in &plan.pages {
        out.push_str(&format!("- :rust:mod:`{}`", page.path));
        let synopsis = summary(&krate.module(page.module).docs);
        if !synopsis.is_empty() {
            out.push_str(" -- ");
            out.push_str(&crate::inline::escape_plain(&synopsis));
        }
        out.push('\n');
    }
    Page { path: "index.rst".to_string(), module: String::new(), text: out, warnings: Vec::new() }
}

/// The groups of a module page, in rustdoc's order.
const GROUPS: [(&str, &[ItemKind]); 11] = [
    ("Macros", &[ItemKind::Macro]),
    ("Structs", &[ItemKind::Struct]),
    ("Enums", &[ItemKind::Enum]),
    ("Unions", &[ItemKind::Union]),
    ("Traits", &[ItemKind::Trait]),
    ("Functions", &[ItemKind::Function]),
    ("Type Aliases", &[ItemKind::TypeAlias]),
    ("Constants", &[ItemKind::Const]),
    ("Statics", &[ItemKind::Static]),
    ("Implementations", &[ItemKind::Impl]),
    ("Other items", &[]),
];

/// One option line of a directive: `:name: value`, or `:name:` for a flag.
type DirectiveOption = (&'static str, String);

/// Writes one page; collects the warnings met on the way.
struct Writer<'k> {
    krate: &'k Crate,
    plan: &'k Plan,
    options: &'k Options,
    out: String,
    warnings: Vec<Diagnostic>,
}

impl Writer<'_> {
    fn module_page(&mut self, page: &PlanPage) {
        let module = self.krate.module(page.module);
        write_title(&page.path, &mut self.out);
        let directive = if page.module == ModuleId(0) { "crate" } else { "module" };
        let mut options = Vec::new();
        let synopsis = summary(&module.docs);
        if !synopsis.is_empty() {
            options.push(("synopsis", synopsis));
        }
        if let Some(item) = module.item.map(|id| self.krate.item(id)) {
            options.extend(common_options(self.krate, item, self.options));
        } else if self.options.noindex {
            options.push(("noindex", String::new()));
        }
        self.directive("", directive, &page.path, &options);
        let docs = module.docs.clone();
        let at = module.item.map_or((module.file, 0), |id| location(self.krate, id));
        let scope = Scope { module: page.module, self_type: None, at: Some(at) };
        self.docs(&docs, "", scope);
        self.re_exports(page);
        self.child_modules(page);
        for (title, kinds) in GROUPS {
            let mut entries: Vec<(ItemId, &str)> = page
                .entries
                .iter()
                .filter_map(|e| match e {
                    Entry::Item { id, name } => Some((*id, name.as_str())),
                    _ => None,
                })
                .filter(|(id, _)| in_group(self.krate.item(*id).kind, kinds))
                .collect();
            if entries.is_empty() {
                continue;
            }
            if self.options.member_order == MemberOrder::Alphabetical {
                entries.sort_by(|a, b| a.1.cmp(b.1));
            }
            self.rubric(title);
            for (id, name) in entries {
                self.item(id, name, "", None);
            }
        }
    }

    fn rubric(&mut self, title: &str) {
        self.out.push_str(".. rubric:: ");
        self.out.push_str(title);
        self.out.push_str("\n\n");
    }

    fn re_exports(&mut self, page: &PlanPage) {
        let uses: Vec<ItemId> = page
            .entries
            .iter()
            .filter_map(|e| match e {
                Entry::Use { id } => Some(*id),
                _ => None,
            })
            .collect();
        if uses.is_empty() {
            return;
        }
        self.rubric("Re-exports");
        for id in uses {
            let item = self.krate.item(id);
            let mut options = common_options(self.krate, item, self.options);
            let target = crate::resolve::follow_use(
                self.krate,
                id,
                Namespace::Any,
                crate::resolve::MAX_HOPS,
            );
            if let Some(path) = target.and_then(|t| self.plan.target_path(self.krate, t)) {
                options.insert(0, ("target", path));
            }
            self.directive("", "use", &item.signature, &options);
            let scope =
                Scope { module: item.module, self_type: None, at: Some(location(self.krate, id)) };
            self.docs(&item.docs, BODY, scope);
        }
    }

    fn child_modules(&mut self, page: &PlanPage) {
        let mut children: Vec<(ModuleId, &str)> = page
            .entries
            .iter()
            .filter_map(|e| match e {
                Entry::Module { id, name } => Some((*id, name.as_str())),
                _ => None,
            })
            .collect();
        if children.is_empty() {
            return;
        }
        if self.options.member_order == MemberOrder::Alphabetical {
            children.sort_by(|a, b| a.1.cmp(b.1));
        }
        self.rubric("Modules");
        for (module, _) in children {
            let Some(path) = self.plan.module_path(module) else { continue };
            self.out.push_str(&format!("- :rust:mod:`{path}`"));
            let synopsis = summary(&self.krate.module(module).docs);
            if !synopsis.is_empty() {
                self.out.push_str(" -- ");
                self.out.push_str(&crate::inline::escape_plain(&synopsis));
            }
            self.out.push('\n');
        }
        self.out.push('\n');
    }

    /// Writes a directive head: `.. rust:name:: signature` and its options, then a blank
    /// line.
    fn directive(
        &mut self,
        indent: &str,
        name: &str,
        signature: &str,
        options: &[DirectiveOption],
    ) {
        self.out.push_str(&format!("{indent}.. rust:{name}:: {signature}\n"));
        for (key, value) in options {
            if value.is_empty() {
                self.out.push_str(&format!("{indent}{BODY}:{key}:\n"));
            } else {
                self.out.push_str(&format!("{indent}{BODY}:{key}: {value}\n"));
            }
        }
        self.out.push('\n');
    }

    fn docs(&mut self, markdown: &str, indent: &str, scope: Scope) {
        if markdown.trim().is_empty() {
            return;
        }
        let mut links =
            ScopeLinks { krate: self.krate, plan: self.plan, scope, warnings: &mut self.warnings };
        write_rst(markdown, indent, &mut links, &mut self.out);
        if !self.out.ends_with("\n\n") {
            self.out.push('\n');
        }
    }

    /// Writes one item's directive, its docs and its members, `name` being the name it
    /// is documented under (an inlining re-export may rename it).
    fn item(&mut self, id: ItemId, name: &str, indent: &str, self_type: Option<ItemId>) {
        let item = self.krate.item(id);
        let signature = renamed(&item.signature, &item.name, name);
        let mut options = common_options(self.krate, item, self.options);
        if item.kind == ItemKind::Impl {
            options.extend(impl_options(item));
        }
        self.directive(indent, item.kind.directive(), &signature, &options);
        let body = format!("{indent}{BODY}");
        if item.kind == ItemKind::Macro {
            self.macro_arms(item, &body);
        }
        let owner = if item.kind.is_type() { Some(id) } else { self_type };
        let scope =
            Scope { module: item.module, self_type: owner, at: Some(location(self.krate, id)) };
        self.docs(&item.docs, &body, scope);
        match item.kind {
            ItemKind::Struct | ItemKind::Union | ItemKind::Variant => self.fields(item, &body),
            ItemKind::Enum => self.members(item, &body, owner, |_| true),
            ItemKind::Trait => self.members(item, &body, owner, |_| true),
            ItemKind::Impl => self.impl_members(item, &body, owner),
            _ => {}
        }
        if item.kind.is_type() {
            self.impls(item, &body, Some(id));
        }
    }

    fn macro_arms(&mut self, item: &Item, body: &str) {
        if item.arms.is_empty() {
            return;
        }
        self.out.push_str(&format!("{body}.. code-block:: rust\n\n"));
        self.out.push_str(&format!("{body}{BODY}{} {{\n", item.signature));
        for arm in &item.arms {
            self.out.push_str(&format!("{body}{BODY}    {arm} => {{ ... }};\n"));
        }
        self.out.push_str(&format!("{body}{BODY}}}\n\n"));
    }

    /// Fields of a struct, union or variant: the shown ones, and tuple fields only when
    /// one of them is documented (rustdoc's rule).
    fn fields(&mut self, item: &Item, body: &str) {
        let is_tuple = item.children.first().is_some_and(|&c| self.krate.item(c).name == "0");
        let has_docs = item.children.iter().any(|&c| !self.krate.item(c).docs.is_empty());
        let in_variant = item.kind == ItemKind::Variant;
        if (is_tuple || in_variant) && !has_docs {
            return;
        }
        self.members(item, body, None, |_| true);
    }

    /// Writes the children of `item` that are shown and pass `keep`.
    fn members(
        &mut self,
        item: &Item,
        body: &str,
        self_type: Option<ItemId>,
        keep: impl Fn(&Item) -> bool,
    ) {
        for &child in &item.children {
            let member = self.krate.item(child);
            if is_shown(member, self.options) && keep(member) {
                let name = member.name.clone();
                self.item(child, &name, body, self_type);
            }
        }
    }

    fn impl_members(&mut self, item: &Item, body: &str, self_type: Option<ItemId>) {
        self.members(item, body, self_type, |_| true);
    }

    /// The impl blocks attached to a type: inherent ones first, then trait impls.
    fn impls(&mut self, item: &Item, body: &str, self_type: Option<ItemId>) {
        let mut blocks: Vec<ItemId> = item
            .impls
            .iter()
            .copied()
            .filter(|&b| {
                let block = self.krate.item(b);
                !block.is_test && (!block.is_hidden || self.options.document_hidden)
            })
            .collect();
        blocks.sort_by_key(|&b| self.krate.item(b).impl_trait.is_some());
        for block in blocks {
            let inherent = self.krate.item(block).impl_trait.is_none();
            let shown_members = self
                .krate
                .item(block)
                .children
                .iter()
                .any(|&c| is_shown(self.krate.item(c), self.options));
            if inherent && !shown_members {
                continue;
            }
            self.item(block, "", body, self_type);
        }
    }
}

fn in_group(kind: ItemKind, kinds: &[ItemKind]) -> bool {
    if kinds.is_empty() {
        // The catch-all group holds what no other group names.
        return !GROUPS.iter().any(|(_, k)| k.contains(&kind));
    }
    kinds.contains(&kind)
}

/// The kind of the item `item` is a member of, if any.
fn parent_kind(krate: &Crate, item: &Item) -> Option<(ItemKind, bool)> {
    let parent = krate.item(item.parent?);
    Some((parent.kind, parent.impl_trait.is_some()))
}

/// The options every directive may carry, from the item's flags.
fn common_options(krate: &Crate, item: &Item, options: &Options) -> Vec<DirectiveOption> {
    let mut out = Vec::new();
    match &item.visibility {
        Visibility::Public | Visibility::Inherited => {}
        Visibility::Private => out.push(("visibility", "private".to_string())),
        other => out.push(("visibility", other.as_str().to_string())),
    }
    match item.cfg.len() {
        0 => {}
        1 => out.push(("cfg", item.cfg[0].clone())),
        _ => out.push(("cfg", format!("all({})", item.cfg.join(", ")))),
    }
    let q = &item.qualifiers;
    let flags = [("unsafe", q.is_unsafe), ("async", q.is_async), ("const", q.is_const)];
    out.extend(flags.into_iter().filter(|f| f.1).map(|f| (f.0, String::new())));
    let parent = parent_kind(krate, item);
    let in_trait = parent.is_some_and(|(kind, _)| kind == ItemKind::Trait);
    let in_trait_impl = parent.is_some_and(|(kind, is_trait)| kind == ItemKind::Impl && is_trait);
    if in_trait && q.is_provided && item.kind != ItemKind::AssociatedType {
        out.push(("provided", String::new()));
    }
    if !item.attributes.is_empty() {
        out.push(("attributes", item.attributes.join(" ")));
    }
    if let Some(note) = &item.deprecated {
        out.push(("deprecated", note.clone()));
    }
    if options.noindex || in_trait_impl || item.kind == ItemKind::Impl {
        out.push(("noindex", String::new()));
    }
    out
}

fn impl_options(item: &Item) -> Vec<DirectiveOption> {
    let mut out = vec![("for", item.impl_type.clone())];
    if let Some(name) = &item.impl_trait {
        out.push(("trait", name.clone()));
    }
    out
}

/// The signature with the item's name replaced by the name it is documented under.
fn renamed(signature: &str, old: &str, new: &str) -> String {
    if old == new || old.is_empty() || new.is_empty() {
        return signature.to_string();
    }
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut from = 0;
    while let Some(found) = signature[from..].find(old) {
        let start = from + found;
        let end = start + old.len();
        let before = signature[..start].chars().last().is_none_or(|c| !is_word(c));
        let after = signature[end..].chars().next().is_none_or(|c| !is_word(c));
        if before && after {
            return format!("{}{}{}", &signature[..start], new, &signature[end..]);
        }
        from = end;
    }
    signature.to_string()
}

/// Where an item is written: its file and byte offset.
fn location(krate: &Crate, id: ItemId) -> (FileId, usize) {
    let item = krate.item(id);
    (item.file, item.offset)
}

/// Where docs are written from: the module their paths are relative to, the type
/// `Self` names, and where they are in the source (for warning locations).
#[derive(Debug, Clone, Copy)]
struct Scope {
    module: ModuleId,
    self_type: Option<ItemId>,
    at: Option<(FileId, usize)>,
}

/// Resolves intra-doc links from one scope and records the links that fail.
struct ScopeLinks<'a> {
    krate: &'a Crate,
    plan: &'a Plan,
    scope: Scope,
    warnings: &'a mut Vec<Diagnostic>,
}

impl ScopeLinks<'_> {
    fn warn(&mut self, code: &'static str, title: &'static str, message: String, hint: &str) {
        let mut warning = Diagnostic::warning(code, title, message, hint);
        if let Some((file, offset)) = self.scope.at {
            let name = self.krate.file_name(file);
            warning = warning.at(&name, self.krate.file_text(file), offset);
        }
        self.warnings.push(warning);
    }

    /// Whether a path that does not resolve is surely meant for this crate (and so a
    /// broken link), rather than for another crate the reader may know.
    fn is_local(&self, path: &str) -> bool {
        let first = path.split("::").next().unwrap_or("");
        if matches!(first, "crate" | "self" | "super" | "Self") || first == self.krate.name {
            return true;
        }
        path.contains("::")
            && resolve_path(
                self.krate,
                self.scope.module,
                self.scope.self_type,
                first,
                Namespace::Any,
            )
            .is_some()
    }
}

impl ScopeLinks<'_> {
    /// Resolves `path`; when it names an undocumented item and no disambiguator was
    /// given, tries each namespace in turn, since Rust keeps modules, types, values and
    /// macros apart (`mod parse;` beside a re-exported `fn parse`).
    fn documented_target(&self, path: &str, namespace: Namespace) -> Option<Target> {
        let (module, self_type) = (self.scope.module, self.scope.self_type);
        let first = resolve_path(self.krate, module, self_type, path, namespace)?;
        if namespace != Namespace::Any || self.plan.target_path(self.krate, first).is_some() {
            return Some(first);
        }
        let others = [Namespace::Type, Namespace::Value, Namespace::Macro, Namespace::Module];
        let documented = others.into_iter().find_map(|ns| {
            resolve_path(self.krate, module, self_type, path, ns)
                .filter(|&t| self.plan.target_path(self.krate, t).is_some())
        });
        Some(documented.unwrap_or(first))
    }
}

impl LinkResolver for ScopeLinks<'_> {
    fn resolve(&mut self, destination: &str, explicit: bool) -> Option<Link> {
        let (path, namespace) = parse_destination(destination)?;
        let target = self.documented_target(&path, namespace);
        let Some(target) = target else {
            if explicit && self.is_local(&path) {
                self.warn(
                    "rust.autodoc.link",
                    "UNRESOLVED INTRA-DOC LINK",
                    format!(
                        "The link `{destination}` names nothing in the crate `{}`.",
                        self.krate.name
                    ),
                    "Correct the path; paths are relative to the item's module, or start \
                     them with `crate::`.",
                );
            }
            return Some(Link::Unresolved { path });
        };
        match self.plan.target_path(self.krate, target) {
            Some(path) => Some(Link::Found { role: role_of(self.krate, target), path }),
            None => {
                if explicit {
                    self.warn(
                        "rust.autodoc.private-link",
                        "LINK TO AN UNDOCUMENTED ITEM",
                        format!(
                            "The link `{destination}` names an item that is not \
                            documented, because it is private or hidden."
                        ),
                        "Link to a public item, make the item public, or set \
                         Options::include_private.",
                    );
                }
                Some(Link::Plain)
            }
        }
    }
}

fn role_of(krate: &Crate, target: Target) -> &'static str {
    match target {
        Target::Module(_) => "mod",
        Target::Item(id) => krate.item(id).kind.role(),
    }
}

/// Splits an intra-doc link destination into a path and the namespace its
/// disambiguator asks for: `struct@Foo`, `foo()`, `foo!`, `Vec<T>` (generics dropped).
/// `None` for primitives (`prim@u8`), which are not items of the crate.
fn parse_destination(destination: &str) -> Option<(String, Namespace)> {
    let mut text = destination.trim().trim_matches('`').trim();
    let mut namespace = Namespace::Any;
    if let Some((prefix, rest)) = text.split_once('@') {
        namespace = match prefix {
            "struct" => Namespace::Kind(ItemKind::Struct),
            "enum" => Namespace::Kind(ItemKind::Enum),
            "trait" => Namespace::Kind(ItemKind::Trait),
            "union" => Namespace::Kind(ItemKind::Union),
            "mod" | "module" => Namespace::Module,
            "fn" | "function" | "method" | "const" | "constant" | "value" => Namespace::Value,
            "static" => Namespace::Kind(ItemKind::Static),
            "type" | "tyalias" => Namespace::Type,
            "macro" | "derive" | "attr" => Namespace::Macro,
            "field" => Namespace::Kind(ItemKind::Field),
            "variant" => Namespace::Kind(ItemKind::Variant),
            "prim" | "primitive" => return None,
            _ => Namespace::Any,
        };
        text = rest;
    }
    if let Some(stripped) = text.strip_suffix("()") {
        text = stripped;
        namespace = Namespace::Value;
    }
    for suffix in ["!()", "![]", "!{}", "!"] {
        if let Some(stripped) = text.strip_suffix(suffix) {
            text = stripped;
            namespace = Namespace::Macro;
            break;
        }
    }
    let mut path = String::with_capacity(text.len());
    let mut depth = 0;
    for c in text.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            _ if depth == 0 && !c.is_whitespace() => path.push(c),
            _ => {}
        }
    }
    (!path.is_empty()).then_some((path, namespace))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(destination: &str) -> (String, Namespace) {
        parse_destination(destination).unwrap_or((String::new(), Namespace::Any))
    }

    #[test]
    fn destinations_keep_their_disambiguators() {
        assert_eq!(parsed("struct@Foo"), ("Foo".to_string(), Namespace::Kind(ItemKind::Struct)));
        assert_eq!(parsed("`foo!`"), ("foo".to_string(), Namespace::Macro));
        assert_eq!(parsed("Vec<T>::new()").0, "Vec::new");
        assert!(parse_destination("prim@u8").is_none());
    }

    #[test]
    fn renaming_replaces_the_name_word() {
        assert_eq!(renamed("pub struct Foo<Foobar>", "Foo", "Bar"), "pub struct Bar<Foobar>");
        assert_eq!(renamed("pub fn new()", "new", "new"), "pub fn new()");
    }
}
