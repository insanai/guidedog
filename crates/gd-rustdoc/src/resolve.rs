//! Name resolution inside the crate: Rust paths (`crate::a::B`, `super::x`, `B::new`)
//! looked up from a module the way rustc looks them up, through `use` imports and glob
//! imports, plus attaching `impl` blocks to the types they implement.
//!
//! Only the crate's own items are known; a path into another crate does not resolve.
//! Chains of imports are followed at most [`MAX_HOPS`] times, so import cycles end.

use crate::model::{Crate, ItemId, ItemKind, ModuleId};

/// How many `use` imports one lookup follows at most.
pub(crate) const MAX_HOPS: usize = 16;

/// What a path names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    Module(ModuleId),
    Item(ItemId),
}

/// A kind filter for the last segment of a path, from an intra-doc link's
/// disambiguator (`struct@Foo`, `foo()`, `foo!`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Namespace {
    Any,
    Type,
    Value,
    Macro,
    Module,
    Kind(ItemKind),
}

impl Namespace {
    fn accepts(self, kind: ItemKind) -> bool {
        match self {
            Namespace::Any => true,
            Namespace::Type => kind.is_type() || kind == ItemKind::AssociatedType,
            Namespace::Value => matches!(
                kind,
                ItemKind::Function
                    | ItemKind::Method
                    | ItemKind::Const
                    | ItemKind::Static
                    | ItemKind::AssociatedConst
                    | ItemKind::Variant
                    | ItemKind::Field
            ),
            Namespace::Macro => kind == ItemKind::Macro,
            Namespace::Module => kind == ItemKind::Module,
            Namespace::Kind(wanted) => kind == wanted,
        }
    }
}

/// Resolves `path` (segments separated by `::`) from module `from`. `self_type` is the
/// type `Self` stands for, when the path is written inside one.
pub(crate) fn resolve_path(
    krate: &Crate,
    from: ModuleId,
    self_type: Option<ItemId>,
    path: &str,
    namespace: Namespace,
) -> Option<Target> {
    let path = path.strip_prefix("::").unwrap_or(path);
    let segments: Vec<&str> = path.split("::").map(str::trim).collect();
    let count = segments.len();
    let mut current: Option<Target> = None;
    for (i, segment) in segments.into_iter().enumerate() {
        let wanted = if i + 1 == count { namespace } else { Namespace::Any };
        current = Some(match current {
            None => first_segment(krate, from, self_type, segment, wanted)?,
            Some(Target::Module(m)) => lookup_name(krate, m, segment, wanted, MAX_HOPS)?,
            Some(Target::Item(item)) => member(krate, item, segment, wanted)?,
        });
    }
    current
}

fn first_segment(
    krate: &Crate,
    from: ModuleId,
    self_type: Option<ItemId>,
    segment: &str,
    wanted: Namespace,
) -> Option<Target> {
    match segment {
        "crate" => Some(Target::Module(ModuleId(0))),
        "self" => Some(Target::Module(from)),
        "super" => krate.module(from).parent.map(Target::Module),
        "Self" => self_type.map(Target::Item),
        _ if segment == krate.name => {
            // The crate's own name works where rustdoc links are written, as in
            // `my_crate::Thing`, unless a local item shadows it.
            lookup_name(krate, from, segment, wanted, MAX_HOPS)
                .or(Some(Target::Module(ModuleId(0))))
        }
        _ => lookup_name(krate, from, segment, wanted, MAX_HOPS)
            .or_else(|| exported_macro(krate, segment, wanted)),
    }
}

/// A `#[macro_export]` macro is usable by name everywhere in the crate.
fn exported_macro(krate: &Crate, name: &str, wanted: Namespace) -> Option<Target> {
    if !matches!(wanted, Namespace::Any | Namespace::Macro | Namespace::Kind(ItemKind::Macro)) {
        return None;
    }
    let found = krate.items.iter().position(|item| {
        item.kind == ItemKind::Macro && item.qualifiers.is_exported && item.name == name
    });
    found.map(|i| Target::Item(ItemId(i as u32)))
}

/// Looks `name` up in module `module`: its own items first, then its explicit imports,
/// then its glob imports, following imports at most `hops` deep.
pub(crate) fn lookup_name(
    krate: &Crate,
    module: ModuleId,
    name: &str,
    wanted: Namespace,
    hops: usize,
) -> Option<Target> {
    let items = &krate.module(module).items;
    for &id in items {
        let item = krate.item(id);
        if item.name != name || item.kind == ItemKind::Use || !wanted.accepts(item.kind) {
            continue;
        }
        return Some(match item.module_target {
            Some(target) => Target::Module(target),
            None => Target::Item(id),
        });
    }
    if hops == 0 {
        return None;
    }
    for &id in items {
        let item = krate.item(id);
        if item.kind == ItemKind::Use
            && item.name == name
            && let Some(found) = follow_use(krate, id, wanted, hops - 1)
        {
            return Some(found);
        }
    }
    for &id in items {
        let item = krate.item(id);
        if item.kind != ItemKind::Use || item.name != "*" {
            continue;
        }
        let base = item.use_path.trim_end_matches("::*");
        let source = resolve_use_path(krate, item.module, base, Namespace::Any, hops - 1);
        if let Some(Target::Module(m)) = source
            && m != module
            && let Some(found) = lookup_name(krate, m, name, wanted, hops - 1)
        {
            return Some(found);
        }
    }
    None
}

/// What a `use` item names (`None` for a path into another crate).
pub(crate) fn follow_use(
    krate: &Crate,
    id: ItemId,
    wanted: Namespace,
    hops: usize,
) -> Option<Target> {
    let item = krate.item(id);
    resolve_use_path(krate, item.module, &item.use_path, wanted, hops)
}

/// Resolves a `use` path: like [`resolve_path`] but without `Self`, and with the hop
/// budget passed down so chains of imports end.
fn resolve_use_path(
    krate: &Crate,
    from: ModuleId,
    path: &str,
    wanted: Namespace,
    hops: usize,
) -> Option<Target> {
    let path = path.strip_prefix("::").unwrap_or(path);
    let segments: Vec<&str> = path.split("::").collect();
    let count = segments.len();
    let mut current: Option<Target> = None;
    for (i, segment) in segments.into_iter().enumerate() {
        let want = if i + 1 == count { wanted } else { Namespace::Any };
        current = Some(match current {
            None => match segment {
                "crate" => Target::Module(ModuleId(0)),
                "self" => Target::Module(from),
                "super" => Target::Module(krate.module(from).parent?),
                _ => lookup_name(krate, from, segment, want, hops)
                    .or_else(|| exported_macro(krate, segment, want))?,
            },
            Some(Target::Module(m)) => match segment {
                "super" => Target::Module(krate.module(m).parent?),
                _ => lookup_name(krate, m, segment, want, hops)?,
            },
            Some(Target::Item(item)) => member(krate, item, segment, want)?,
        });
    }
    current
}

/// A member of an item: a variant or associated item, then a method or associated item of
/// one of the impls attached to a type (inherent impls before trait impls), then a field.
/// Fields come last, as in rustdoc, so `Canvas::width` names the method `width` rather
/// than a field of the same name.
pub(crate) fn member(
    krate: &Crate,
    owner: ItemId,
    name: &str,
    wanted: Namespace,
) -> Option<Target> {
    let item = krate.item(owner);
    let matches = |id: &ItemId, fields: bool| {
        let child = krate.item(*id);
        child.name == name
            && wanted.accepts(child.kind)
            && (child.kind == ItemKind::Field) == fields
    };
    if let Some(&found) = item.children.iter().find(|id| matches(id, false)) {
        return Some(Target::Item(found));
    }
    let mut impls: Vec<ItemId> = item.impls.clone();
    impls.sort_by_key(|&i| krate.item(i).impl_trait.is_some());
    let in_impl = impls
        .iter()
        .find_map(|&i| krate.item(i).children.iter().copied().find(|id| matches(id, false)));
    in_impl
        .or_else(|| item.children.iter().copied().find(|id| matches(id, true)))
        .map(Target::Item)
}

/// The type an impl block's self type names, as a path a lookup can take:
/// `&'a mut foo::Bar<T>` gives `foo::Bar`.
pub(crate) fn type_path(written: &str) -> &str {
    let mut text = written.trim();
    loop {
        let before = text;
        text = text.trim_start_matches('&').trim_start();
        if text.starts_with('\'') {
            text = text.split_once(' ').map_or("", |(_, rest)| rest).trim_start();
        }
        for prefix in ["mut ", "dyn ", "const "] {
            text = text.strip_prefix(prefix).unwrap_or(text);
        }
        if text == before {
            break;
        }
    }
    let end = text.find(['<', ' ', '(', '[']).unwrap_or(text.len());
    &text[..end]
}

/// Attaches every impl block to the crate type it implements, when that type is found:
/// the block then shows on the type's description. Others stay in their module. A
/// trait impl is also listed on the crate trait it implements, when that is found.
pub(crate) fn attach_impls(krate: &mut Crate) {
    let mut attached = Vec::new();
    let mut implemented = Vec::new();
    for (index, item) in krate.items.iter().enumerate() {
        if item.kind != ItemKind::Impl {
            continue;
        }
        let block = ItemId(index as u32);
        if let Some(owner) = impl_target(krate, item.module, &item.impl_type, Namespace::Type) {
            attached.push((owner, block));
        }
        let trait_path = item.impl_trait.as_deref().map(|t| t.trim_start_matches('!'));
        let trait_kind = Namespace::Kind(ItemKind::Trait);
        if let Some(found) =
            trait_path.and_then(|t| impl_target(krate, item.module, t, trait_kind))
        {
            implemented.push((found, block));
        }
    }
    for (owner, block) in attached {
        krate.items[owner.0 as usize].impls.push(block);
    }
    for (trait_id, block) in implemented {
        krate.items[trait_id.0 as usize].implementors.push(block);
    }
}

/// The crate item a type or trait written in an impl header names, from `module`.
fn impl_target(
    krate: &Crate,
    module: ModuleId,
    written: &str,
    namespace: Namespace,
) -> Option<ItemId> {
    let path = type_path(written);
    if path.is_empty() {
        return None;
    }
    match resolve_path(krate, module, None, path, namespace) {
        Some(Target::Item(id)) => Some(id),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_paths_drop_references_and_generics() {
        assert_eq!(type_path("&'a mut foo::Bar<T>"), "foo::Bar");
        assert_eq!(type_path("Point<T>"), "Point");
        assert_eq!(type_path("dyn Trait + Send"), "Trait");
        assert_eq!(type_path("[u8]"), "");
    }
}
