//! Reading one parsed Rust file into the crate model: its items, their members, the
//! inline modules it declares, and the file modules it asks the loader to read next.
//!
//! The walk is flat: each declaration list (a file, an inline module's body, an impl's
//! body) is visited with a cursor over its children, and inline modules go on an explicit
//! stack, so no Rust recursion follows the source's nesting.

use crate::attrs::{Attrs, Include};
use crate::model::{Crate, FileId, Item, ItemId, ItemKind, Module, ModuleId, Visibility};
use crate::sig::{normalize, signature, trim_value};
use std::path::{Path, PathBuf};
use tree_sitter::Node;

/// A `mod name;` whose file the loader still has to find and read.
#[derive(Debug, Clone)]
pub(crate) struct PendingModule {
    /// The module the declaration created (still empty).
    pub module: ModuleId,
    /// The folder `name.rs` and `name/mod.rs` are looked for in.
    pub dir: PathBuf,
    /// The folder a `#[path]` attribute is relative to.
    pub path_base: PathBuf,
    /// The module's name.
    pub name: String,
    /// The value of a `#[path = "..."]` attribute.
    pub path_attr: Option<String>,
    /// The declaration's byte offset, for warnings.
    pub offset: usize,
    /// The file holding the declaration (`None` for a loose file of a root-less folder).
    pub file: Option<FileId>,
}

/// The state of reading one file.
pub(crate) struct FileReader<'a> {
    pub krate: &'a mut Crate,
    pub source: &'a str,
    pub file: FileId,
    pub include: &'a mut Include<'a>,
    /// File modules found, for the loader.
    pub pending: Vec<PendingModule>,
    /// How many more items may be created; reading stops adding items at zero.
    pub item_budget: &'a mut usize,
    /// Set when the budget ran out.
    pub truncated: bool,
}

/// Where a list of members sits, which decides their kind and default visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Container {
    Trait,
    InherentImpl,
    TraitImpl,
}

impl<'a> FileReader<'a> {
    /// Reads the items of `root` (a `source_file`) into `module`, whose child modules'
    /// files are looked for in `dir`; inline modules are read too. Inner doc comments
    /// and attributes of the file belong to `module`.
    pub fn read_file(&mut self, root: Node<'a>, module: ModuleId, dir: PathBuf) {
        let mut stack = vec![(root, module, dir)];
        while let Some((list, module, dir)) = stack.pop() {
            let inner = self.read_list(list, module, &dir, &mut stack);
            self.apply_inner(module, inner);
        }
    }

    /// Adds a module's inner docs and `cfg`s, after the docs of its `mod` item.
    fn apply_inner(&mut self, module: ModuleId, mut inner: Attrs) {
        let docs = inner.docs();
        let m = &mut self.krate.modules[module.0 as usize];
        if !docs.is_empty() {
            if !m.docs.is_empty() {
                m.docs.push_str("\n\n");
            }
            m.docs.push_str(&docs);
        }
        m.cfg.append(&mut inner.cfg);
        if (inner.is_hidden || inner.is_test)
            && let Some(item) = m.item
        {
            let item = &mut self.krate.items[item.0 as usize];
            item.is_hidden |= inner.is_hidden;
            item.is_test |= inner.is_test;
        }
    }

    /// Reads one declaration list into `module`; returns the inner attributes found.
    fn read_list(
        &mut self,
        list: Node<'a>,
        module: ModuleId,
        dir: &Path,
        stack: &mut Vec<(Node<'a>, ModuleId, PathBuf)>,
    ) -> Attrs {
        let mut inner = Attrs::default();
        let mut attrs = Attrs::default();
        let mut cursor = list.walk();
        for node in list.children(&mut cursor) {
            match node.kind() {
                "line_comment" | "block_comment" => {
                    inner.add_comment(node, self.source, true);
                    attrs.add_comment(node, self.source, false);
                }
                "inner_attribute_item" => self.add_attribute(&mut inner, node),
                "attribute_item" => self.add_attribute(&mut attrs, node),
                _ if node.is_named() => {
                    let taken = std::mem::take(&mut attrs);
                    if let Some((body, child)) = self.read_item(node, taken, module, dir) {
                        stack.push((body, child, dir.join(&self.krate.module(child).name)));
                    }
                }
                _ => {}
            }
        }
        inner
    }

    fn add_attribute(&mut self, attrs: &mut Attrs, node: Node) {
        if let Some(attr) = node.named_child(0) {
            attrs.add_attribute(attr, self.source, self.include);
        }
    }

    /// Reads one declaration. Returns the body and module of an inline `mod`, which the
    /// caller reads next.
    fn read_item(
        &mut self,
        node: Node<'a>,
        attrs: Attrs,
        module: ModuleId,
        dir: &Path,
    ) -> Option<(Node<'a>, ModuleId)> {
        match node.kind() {
            "function_item" | "function_signature_item" => {
                self.read_function(node, attrs, module, None, ItemKind::Function);
            }
            "struct_item" | "union_item" => self.read_struct(node, attrs, module),
            "enum_item" => self.read_enum(node, attrs, module),
            "trait_item" => self.read_trait(node, attrs, module),
            "impl_item" => self.read_impl(node, attrs, module),
            "type_item" => self.read_simple(node, attrs, module, None, ItemKind::TypeAlias),
            "const_item" => self.read_simple(node, attrs, module, None, ItemKind::Const),
            "static_item" => self.read_simple(node, attrs, module, None, ItemKind::Static),
            "macro_definition" => self.read_macro(node, attrs, module),
            "use_declaration" => self.read_use(node, attrs, module),
            "foreign_mod_item" => self.read_foreign(node, module),
            "mod_item" => return self.read_mod(node, attrs, module, dir),
            _ => {}
        }
        None
    }

    /// Creates an item from its parts and the attributes before it; `None` once the
    /// item budget is spent.
    fn add(&mut self, mut item: Item, mut attrs: Attrs, node: Node) -> Option<ItemId> {
        if *self.item_budget == 0 {
            self.truncated = true;
            return None;
        }
        *self.item_budget -= 1;
        item.docs = attrs.docs();
        item.cfg = std::mem::take(&mut attrs.cfg);
        item.attributes = std::mem::take(&mut attrs.attributes);
        item.deprecated = attrs.deprecated.take();
        item.is_hidden = attrs.is_hidden;
        item.is_test = attrs.is_test;
        item.inline = attrs.inline;
        item.qualifiers.is_exported = attrs.is_exported;
        if attrs.is_exported {
            // `#[macro_export]` makes a macro public at the crate root.
            item.visibility = Visibility::Public;
        }
        item.file = self.file;
        item.offset = node.start_byte();
        let parent = item.parent;
        let module = item.module;
        let id = self.krate.add_item(item);
        match parent {
            Some(p) => self.krate.items[p.0 as usize].children.push(id),
            None => self.krate.modules[module.0 as usize].items.push(id),
        }
        Some(id)
    }

    /// The text of `node`'s `field` child, or "".
    fn field_text(&self, node: Node, field: &str) -> &'a str {
        node.child_by_field_name(field).map_or("", |n| &self.source[n.byte_range()])
    }

    fn visibility(&self, node: Node, default: Visibility) -> Visibility {
        let mut cursor = node.walk();
        let found = node.children(&mut cursor).find(|c| c.kind() == "visibility_modifier");
        match found {
            Some(modifier) => visibility_of(&self.source[modifier.byte_range()]),
            None => default,
        }
    }

    fn item(&self, kind: ItemKind, node: Node, module: ModuleId, parent: Option<ItemId>) -> Item {
        Item {
            kind,
            name: self.field_text(node, "name").to_string(),
            visibility: self.visibility(node, Visibility::Private),
            module,
            parent,
            ..Item::default()
        }
    }

    fn read_function(
        &mut self,
        node: Node,
        attrs: Attrs,
        module: ModuleId,
        parent: Option<(ItemId, Container)>,
        kind: ItemKind,
    ) {
        let mut item = self.item(kind, node, module, parent.map(|p| p.0));
        if matches!(parent, Some((_, Container::Trait | Container::TraitImpl))) {
            item.visibility = Visibility::Inherited;
        }
        let body = node.child_by_field_name("body");
        let cut: Vec<_> = body.iter().map(|b| b.byte_range()).collect();
        item.signature = signature(self.source, node, &cut);
        item.qualifiers.is_provided = body.is_some();
        let mut cursor = node.walk();
        let modifiers = node.children(&mut cursor).find(|c| c.kind() == "function_modifiers");
        if let Some(modifiers) = modifiers {
            let mut inner = modifiers.walk();
            for m in modifiers.children(&mut inner) {
                match m.kind() {
                    "unsafe" => item.qualifiers.is_unsafe = true,
                    "async" => item.qualifiers.is_async = true,
                    "const" => item.qualifiers.is_const = true,
                    "extern_modifier" => item.qualifiers.abi = abi_of(m, self.source),
                    _ => {}
                }
            }
        }
        self.add(item, attrs, node);
    }

    /// Reads a type alias, const, static or associated type: a signature, no members.
    fn read_simple(
        &mut self,
        node: Node,
        attrs: Attrs,
        module: ModuleId,
        parent: Option<(ItemId, Container)>,
        kind: ItemKind,
    ) {
        let mut item = self.item(kind, node, module, parent.map(|p| p.0));
        if matches!(parent, Some((_, Container::Trait | Container::TraitImpl))) {
            item.visibility = Visibility::Inherited;
        }
        let written = signature(self.source, node, &[]);
        let has_value = node.child_by_field_name("value").is_some()
            || (node.kind() == "type_item" && node.child_by_field_name("type").is_some());
        item.qualifiers.is_provided = has_value;
        item.signature = match kind {
            ItemKind::Static => trim_value(&written, true),
            ItemKind::Const | ItemKind::AssociatedConst => trim_value(&written, false),
            _ => written,
        };
        let mut cursor = node.walk();
        item.qualifiers.is_mut =
            node.children(&mut cursor).any(|c| c.kind() == "mutable_specifier");
        self.add(item, attrs, node);
    }

    fn read_struct(&mut self, node: Node, attrs: Attrs, module: ModuleId) {
        let kind = if node.kind() == "union_item" { ItemKind::Union } else { ItemKind::Struct };
        let mut item = self.item(kind, node, module, None);
        let body = node.child_by_field_name("body");
        let cut: Vec<_> = body.iter().map(|b| b.byte_range()).collect();
        item.signature = signature(self.source, node, &cut);
        let tuple = body.filter(|b| b.kind() == "ordered_field_declaration_list");
        if let Some(tuple) = tuple {
            item.signature = with_tuple(&item.signature, &self.tuple_shape(tuple));
        }
        let Some(id) = self.add(item, attrs, node) else { return };
        if let Some(body) = body {
            self.read_fields(body, id, module);
        }
    }

    /// The tuple a tuple struct shows: public fields' types, `_` for the others.
    fn tuple_shape(&self, list: Node) -> String {
        let mut parts = Vec::new();
        let mut public = false;
        for (child, field) in children_with_fields(list) {
            match child.kind() {
                "visibility_modifier" => {
                    public = normalize(&self.source[child.byte_range()]) == "pub";
                }
                _ if field == Some("type") => {
                    let shown = if public {
                        format!("pub {}", normalize(&self.source[child.byte_range()]))
                    } else {
                        "_".to_string()
                    };
                    parts.push(shown);
                    public = false;
                }
                _ => {}
            }
        }
        format!("({})", parts.join(", "))
    }

    /// Reads the fields of a struct, union or variant body as children of `parent`.
    fn read_fields(&mut self, body: Node, parent: ItemId, module: ModuleId) {
        let default = if self.krate.item(parent).kind == ItemKind::Variant {
            Visibility::Inherited
        } else {
            Visibility::Private
        };
        if body.kind() == "ordered_field_declaration_list" {
            self.read_tuple_fields(body, parent, module, default);
            return;
        }
        let mut attrs = Attrs::default();
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                "line_comment" | "block_comment" => attrs.add_comment(child, self.source, false),
                "attribute_item" => self.add_attribute(&mut attrs, child),
                "field_declaration" => {
                    let mut item = self.item(ItemKind::Field, child, module, Some(parent));
                    item.visibility = self.visibility(child, default.clone());
                    item.signature = signature(self.source, child, &[]);
                    self.add(item, std::mem::take(&mut attrs), child);
                }
                _ => {}
            }
        }
    }

    fn read_tuple_fields(
        &mut self,
        body: Node,
        parent: ItemId,
        module: ModuleId,
        default: Visibility,
    ) {
        let mut attrs = Attrs::default();
        let mut visibility = default.clone();
        let mut index = 0;
        for (child, field) in children_with_fields(body) {
            match child.kind() {
                "line_comment" | "block_comment" => attrs.add_comment(child, self.source, false),
                "attribute_item" => self.add_attribute(&mut attrs, child),
                "visibility_modifier" => {
                    visibility = visibility_of(&self.source[child.byte_range()]);
                }
                _ if field == Some("type") => {
                    let ty = normalize(&self.source[child.byte_range()]);
                    let shown = visibility.as_str();
                    let space = if shown.is_empty() { "" } else { " " };
                    let item = Item {
                        kind: ItemKind::Field,
                        name: index.to_string(),
                        signature: format!("{shown}{space}{index}: {ty}"),
                        visibility: std::mem::replace(&mut visibility, default.clone()),
                        module,
                        parent: Some(parent),
                        ..Item::default()
                    };
                    self.add(item, std::mem::take(&mut attrs), child);
                    index += 1;
                }
                _ => {}
            }
        }
    }

    fn read_enum(&mut self, node: Node, attrs: Attrs, module: ModuleId) {
        let mut item = self.item(ItemKind::Enum, node, module, None);
        let body = node.child_by_field_name("body");
        let cut: Vec<_> = body.iter().map(|b| b.byte_range()).collect();
        item.signature = signature(self.source, node, &cut);
        let Some(id) = self.add(item, attrs, node) else { return };
        let Some(body) = body else { return };
        let mut attrs = Attrs::default();
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                "line_comment" | "block_comment" => attrs.add_comment(child, self.source, false),
                "attribute_item" => self.add_attribute(&mut attrs, child),
                "enum_variant" => {
                    let mut variant = self.item(ItemKind::Variant, child, module, Some(id));
                    variant.visibility = Visibility::Inherited;
                    variant.signature = signature(self.source, child, &[]);
                    let taken = std::mem::take(&mut attrs);
                    let Some(vid) = self.add(variant, taken, child) else { return };
                    if let Some(fields) = child.child_by_field_name("body") {
                        self.read_fields(fields, vid, module);
                    }
                }
                _ => {}
            }
        }
    }

    fn read_trait(&mut self, node: Node, attrs: Attrs, module: ModuleId) {
        let mut item = self.item(ItemKind::Trait, node, module, None);
        let body = node.child_by_field_name("body");
        let cut: Vec<_> = body.iter().map(|b| b.byte_range()).collect();
        item.signature = signature(self.source, node, &cut);
        let mut cursor = node.walk();
        item.qualifiers.is_unsafe = node.children(&mut cursor).any(|c| c.kind() == "unsafe");
        let Some(id) = self.add(item, attrs, node) else { return };
        if let Some(body) = body {
            self.read_members(body, id, module, Container::Trait);
        }
    }

    fn read_impl(&mut self, node: Node, attrs: Attrs, module: ModuleId) {
        let mut item = self.item(ItemKind::Impl, node, module, None);
        item.name = String::new();
        item.visibility = Visibility::Inherited;
        let body = node.child_by_field_name("body");
        let cut: Vec<_> = body.iter().map(|b| b.byte_range()).collect();
        item.signature = signature(self.source, node, &cut);
        item.impl_type = normalize(self.field_text(node, "type"));
        let trait_text = self.field_text(node, "trait");
        let mut cursor = node.walk();
        let negative = node.children(&mut cursor).any(|c| c.kind() == "!");
        if !trait_text.is_empty() {
            let bang = if negative { "!" } else { "" };
            item.impl_trait = Some(format!("{bang}{}", normalize(trait_text)));
        }
        let mut cursor = node.walk();
        item.qualifiers.is_unsafe = node.children(&mut cursor).any(|c| c.kind() == "unsafe");
        let container =
            if item.impl_trait.is_some() { Container::TraitImpl } else { Container::InherentImpl };
        let Some(id) = self.add(item, attrs, node) else { return };
        if let Some(body) = body {
            self.read_members(body, id, module, container);
        }
    }

    /// Reads the associated items of a trait or impl body.
    fn read_members(
        &mut self,
        body: Node,
        parent: ItemId,
        module: ModuleId,
        container: Container,
    ) {
        let mut attrs = Attrs::default();
        let mut cursor = body.walk();
        let owner = Some((parent, container));
        for child in body.children(&mut cursor) {
            let kind = child.kind();
            match kind {
                "line_comment" | "block_comment" => attrs.add_comment(child, self.source, false),
                "attribute_item" => self.add_attribute(&mut attrs, child),
                "function_item" | "function_signature_item" => {
                    let taken = std::mem::take(&mut attrs);
                    self.read_function(child, taken, module, owner, ItemKind::Method);
                }
                "const_item" | "type_item" | "associated_type" => {
                    let item_kind = if kind == "const_item" {
                        ItemKind::AssociatedConst
                    } else {
                        ItemKind::AssociatedType
                    };
                    let taken = std::mem::take(&mut attrs);
                    self.read_simple(child, taken, module, owner, item_kind);
                }
                _ if child.is_named() => attrs = Attrs::default(),
                _ => {}
            }
        }
    }

    fn read_macro(&mut self, node: Node, attrs: Attrs, module: ModuleId) {
        let mut item = self.item(ItemKind::Macro, node, module, None);
        item.signature = format!("macro_rules! {}", item.name);
        let mut cursor = node.walk();
        for rule in node.children(&mut cursor).filter(|c| c.kind() == "macro_rule") {
            item.arms.push(normalize(self.field_text(rule, "left")));
        }
        self.add(item, attrs, node);
    }

    /// Reads `extern "ABI" { ... }`: its functions are shown as `unsafe extern "ABI" fn`.
    fn read_foreign(&mut self, node: Node, module: ModuleId) {
        let mut cursor = node.walk();
        let abi = node
            .children(&mut cursor)
            .find(|c| c.kind() == "extern_modifier")
            .map(|m| abi_of(m, self.source))
            .unwrap_or_else(|| "C".to_string());
        let Some(body) = node.child_by_field_name("body") else { return };
        let mut attrs = Attrs::default();
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                "line_comment" | "block_comment" => attrs.add_comment(child, self.source, false),
                "attribute_item" => self.add_attribute(&mut attrs, child),
                "function_signature_item" => {
                    let taken = std::mem::take(&mut attrs);
                    self.read_function(child, taken, module, None, ItemKind::Function);
                    self.mark_foreign(&abi);
                }
                "static_item" => {
                    let taken = std::mem::take(&mut attrs);
                    self.read_simple(child, taken, module, None, ItemKind::Static);
                }
                _ => {}
            }
        }
    }

    /// Rewrites the function just added as a foreign one: unsafe, with the block's ABI.
    fn mark_foreign(&mut self, abi: &str) {
        let Some(item) = self.krate.items.last_mut() else { return };
        if item.kind != ItemKind::Function || item.qualifiers.is_unsafe {
            return;
        }
        item.qualifiers.is_unsafe = true;
        item.qualifiers.abi = abi.to_string();
        let visibility = item.visibility.as_str().to_string();
        let rest = item.signature.strip_prefix(&visibility).unwrap_or(&item.signature).trim();
        let space = if visibility.is_empty() { "" } else { " " };
        item.signature = format!("{visibility}{space}unsafe extern \"{abi}\" {rest}");
    }

    fn read_mod(
        &mut self,
        node: Node<'a>,
        attrs: Attrs,
        parent: ModuleId,
        dir: &Path,
    ) -> Option<(Node<'a>, ModuleId)> {
        let mut item = self.item(ItemKind::Module, node, parent, None);
        item.signature = format!("mod {}", item.name);
        let name = item.name.clone();
        let path_attr = attrs.path.clone();
        let id = self.add(item, attrs, node)?;
        let module = self.krate.add_module(Module {
            name: name.clone(),
            parent: Some(parent),
            item: Some(id),
            docs: self.krate.item(id).docs.clone(),
            file: self.file,
            ..Module::default()
        });
        self.krate.items[id.0 as usize].module_target = Some(module);
        if let Some(body) = node.child_by_field_name("body") {
            return Some((body, module));
        }
        self.pending.push(PendingModule {
            module,
            dir: dir.to_path_buf(),
            path_base: dir.to_path_buf(),
            name,
            path_attr,
            offset: node.start_byte(),
            file: Some(self.file),
        });
        None
    }

    fn read_use(&mut self, node: Node, attrs: Attrs, module: ModuleId) {
        let Some(argument) = node.child_by_field_name("argument") else { return };
        let visibility = self.visibility(node, Visibility::Private);
        let mut entries = Vec::new();
        flatten_use(argument, self.source, &mut entries);
        let attrs_template = attrs;
        let mut first = Some(attrs_template);
        for (path, name) in entries {
            let shown = visibility.as_str();
            let alias = if path.ends_with(&format!("::{name}")) || path == name || name == "*" {
                String::new()
            } else {
                format!(" as {name}")
            };
            let space = if shown.is_empty() { "" } else { " " };
            let item = Item {
                kind: ItemKind::Use,
                signature: format!("{shown}{space}use {path}{alias}"),
                name,
                use_path: path,
                visibility: visibility.clone(),
                module,
                ..Item::default()
            };
            // Docs and attributes belong to the declaration; every name it brings in
            // shares them, but the docs are kept once, on the first.
            let attrs = first.take().unwrap_or_default();
            if let Some(id) = self.add(item, attrs, node) {
                self.copy_flags(id);
            }
        }
    }

    /// Copies the hidden and inline flags of the first `use` entry of a declaration to the
    /// later ones (the docs stay on the first).
    fn copy_flags(&mut self, id: ItemId) {
        let index = id.0 as usize;
        let Some(previous) = index.checked_sub(1).map(|i| &self.krate.items[i]) else { return };
        let same =
            previous.kind == ItemKind::Use && previous.offset == self.krate.items[index].offset;
        if same {
            let (hidden, inline, cfg) =
                (previous.is_hidden, previous.inline, previous.cfg.clone());
            let item = &mut self.krate.items[index];
            item.is_hidden |= hidden;
            item.inline = item.inline.or(inline);
            if item.cfg.is_empty() {
                item.cfg = cfg;
            }
        }
    }
}

/// The children of `node` with the field name each has in its parent.
fn children_with_fields(node: Node) -> Vec<(Node, Option<&'static str>)> {
    let mut out = Vec::with_capacity(node.child_count());
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            out.push((cursor.node(), cursor.field_name()));
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    out
}

/// The ABI string of an `extern_modifier` node, `C` when it names none.
fn abi_of(modifier: Node, source: &str) -> String {
    let mut cursor = modifier.walk();
    let found = modifier.children(&mut cursor).find(|c| c.kind() == "string_literal");
    found.map_or_else(|| "C".to_string(), |s| crate::attrs::string_value(&source[s.byte_range()]))
}

/// The visibility a modifier's text (`pub`, `pub(crate)`, ...) gives.
fn visibility_of(modifier: &str) -> Visibility {
    let compact: String = modifier.chars().filter(|c| !c.is_whitespace()).collect();
    match compact.as_str() {
        "pub" => Visibility::Public,
        "pub(crate)" | "crate" => Visibility::Crate,
        "pub(super)" => Visibility::Super,
        "pub(self)" => Visibility::Private,
        _ => Visibility::Restricted(normalize(modifier)),
    }
}

/// Puts a tuple struct's field list after its generics, before any `where` clause.
fn with_tuple(header: &str, tuple: &str) -> String {
    match header.find(" where ") {
        Some(at) => format!("{}{}{}", &header[..at], tuple, &header[at..]),
        None => format!("{header}{tuple}"),
    }
}

/// Flattens a use tree into `(path, name)` pairs: `a::{b as c, d::*}` gives
/// `("a::b", "c")` and `("a::d::*", "*")`. Walks with an explicit stack.
fn flatten_use(argument: Node, source: &str, out: &mut Vec<(String, String)>) {
    let mut stack = vec![(String::new(), argument)];
    while let Some((prefix, node)) = stack.pop() {
        let join = |text: &str| -> String {
            let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
            if prefix.is_empty() { text } else { format!("{prefix}::{text}") }
        };
        match node.kind() {
            "use_as_clause" => {
                let path =
                    node.child_by_field_name("path").map_or("", |p| &source[p.byte_range()]);
                let alias =
                    node.child_by_field_name("alias").map_or("", |a| &source[a.byte_range()]);
                out.push((join(path), alias.to_string()));
            }
            "use_wildcard" => out.push((join(&source[node.byte_range()]), "*".to_string())),
            "scoped_use_list" => {
                let path =
                    node.child_by_field_name("path").map_or("", |p| &source[p.byte_range()]);
                let next = if path.is_empty() { prefix.clone() } else { join(path) };
                if let Some(list) = node.child_by_field_name("list") {
                    push_list(list, next, &mut stack);
                }
            }
            "use_list" => push_list(node, prefix.clone(), &mut stack),
            _ => {
                let path = join(&source[node.byte_range()]);
                out.push(use_entry(path));
            }
        }
    }
    // The stack visits lists back to front; restore the written order.
    out.reverse();
}

fn push_list<'t>(list: Node<'t>, prefix: String, stack: &mut Vec<(String, Node<'t>)>) {
    let mut cursor = list.walk();
    // Pushed in written order, so they are popped and recorded last first; flatten_use
    // reverses its whole output at the end, which restores the written order.
    let paths = list.named_children(&mut cursor).filter(|c| !c.kind().ends_with("comment"));
    for child in paths {
        stack.push((prefix.clone(), child));
    }
}

/// The name a plain use path brings in: its last segment, or for `a::self` the one
/// before it.
fn use_entry(path: String) -> (String, String) {
    let trimmed = path.strip_suffix("::self").unwrap_or(&path).to_string();
    let name = trimmed.rsplit("::").next().unwrap_or(&trimmed).to_string();
    (trimmed, name)
}
