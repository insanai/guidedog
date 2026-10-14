//! Loading a crate: finding its root file, following `mod name;` declarations to their
//! files, parsing each file with tree-sitter and reading it into the model. Files are
//! read only below the crate's folder and within the size and count limits.

use crate::Options;
use crate::diag::{Diagnostic, Error};
use crate::model::{Crate, FileId, Item, ItemKind, Module, ModuleId, SourceFile, Visibility};
use crate::parse::{FileReader, PendingModule};
use crate::resolve;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

/// Where a crate starts: its name, its folder, and its root file (`None` for a folder of
/// `.rs` files without a root, whose files become the root's modules).
#[derive(Debug)]
struct Located {
    name: String,
    root_dir: PathBuf,
    root_file: Option<PathBuf>,
    loose_files: Vec<PathBuf>,
}

/// Loads the crate at `path`: a folder with `Cargo.toml` (its `[lib] path`,
/// `src/lib.rs` or `src/main.rs`), a folder with `lib.rs`, `main.rs` or `mod.rs`, a
/// folder of `.rs` files (each a public module), or one root `.rs` file. Module files
/// are followed as rustc follows them, `#[path]` included.
///
/// Fails when the root cannot be found or read. Problems below the root (a missing
/// module file, a syntax error, a limit reached) are warnings in [`Crate::warnings`],
/// and what could be read is kept.
pub fn load_crate(path: &Path, options: &Options) -> Result<Crate, Error> {
    let located = locate(path, options)?;
    let mut krate = Crate {
        name: located.name.clone(),
        root_dir: located.root_dir.clone(),
        ..Crate::default()
    };
    krate.add_module(Module { name: located.name.clone(), ..Module::default() });
    let mut loader = Loader::new(options)?;
    match &located.root_file {
        Some(root) => {
            let text = loader.read_root(&krate, root)?;
            let dir = root.parent().map(Path::to_path_buf).unwrap_or_default();
            loader.read_module(&mut krate, ModuleId(0), root, text, dir);
        }
        None => loader.queue_loose_files(&mut krate, &located.loose_files),
    }
    loader.read_pending(&mut krate);
    resolve::attach_impls(&mut krate);
    Ok(krate)
}

/// A file parsed and about to be read into a module.
struct ParsedFile<'t> {
    module: ModuleId,
    file: FileId,
    path: &'t Path,
    text: &'t str,
    root: tree_sitter::Node<'t>,
    /// The folder the file's `mod name;` declarations are looked for in.
    dir: PathBuf,
}

/// The state of one load: the parser, the queue of module files, and the limits.
struct Loader<'o> {
    options: &'o Options,
    parser: tree_sitter::Parser,
    queue: VecDeque<PendingModule>,
    seen: HashSet<PathBuf>,
    item_budget: usize,
    warned_items: bool,
}

impl<'o> Loader<'o> {
    fn new(options: &'o Options) -> Result<Self, Error> {
        let mut parser = tree_sitter::Parser::new();
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        if parser.set_language(&language).is_err() {
            return Err(Error::new(
                "rust.autodoc.grammar",
                "RUST GRAMMAR NOT USABLE",
                "The tree-sitter Rust grammar does not match the tree-sitter library.".into(),
                "Pair tree-sitter and tree-sitter-rust versions in [workspace.dependencies].",
            ));
        }
        Ok(Loader {
            options,
            parser,
            queue: VecDeque::new(),
            seen: HashSet::new(),
            item_budget: options.max_items,
            warned_items: false,
        })
    }

    /// Reads the crate root; failing here fails the whole load.
    fn read_root(&mut self, krate: &Crate, path: &Path) -> Result<String, Error> {
        self.seen.insert(canonical(path));
        read_source(path, &krate.root_dir, self.options).map_err(|d| Error::from(*d))
    }

    /// Makes each loose `.rs` file of a root-less folder a public module of the root.
    fn queue_loose_files(&mut self, krate: &mut Crate, files: &[PathBuf]) {
        for file in files {
            let name =
                file.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let item = krate.add_item(Item {
                kind: ItemKind::Module,
                signature: format!("pub mod {name}"),
                name: name.clone(),
                visibility: Visibility::Public,
                ..Item::default()
            });
            let module = krate.add_module(Module {
                name: name.clone(),
                parent: Some(ModuleId(0)),
                item: Some(item),
                ..Module::default()
            });
            krate.items[item.0 as usize].module_target = Some(module);
            krate.modules[0].items.push(item);
            let dir = file.parent().map(Path::to_path_buf).unwrap_or_default();
            self.queue.push_back(PendingModule {
                module,
                dir: dir.clone(),
                path_base: dir,
                name,
                path_attr: file.file_name().map(|f| f.to_string_lossy().to_string()),
                offset: 0,
                file: None,
            });
        }
    }

    /// Reads queued module files until none is left or the file limit is reached.
    fn read_pending(&mut self, krate: &mut Crate) {
        while let Some(pending) = self.queue.pop_front() {
            if krate.files.len() >= self.options.max_files {
                let message = format!(
                    "The crate has more than {} source files; the module `{}` and the rest \
                     are left out.",
                    self.options.max_files, pending.name
                );
                krate.warnings.push(Diagnostic::warning(
                    "rust.autodoc.limit",
                    "TOO MANY RUST FILES",
                    message,
                    "Raise Options::max_files, or document a smaller crate.",
                ));
                return;
            }
            self.read_pending_module(krate, pending);
        }
    }

    fn read_pending_module(&mut self, krate: &mut Crate, pending: PendingModule) {
        let Some((path, mod_rs)) = find_module_file(&pending) else {
            krate.warnings.push(missing_module(krate, &pending));
            return;
        };
        if !self.seen.insert(canonical(&path)) {
            return;
        }
        match read_source(&path, &krate.root_dir, self.options) {
            Ok(text) => {
                let dir = children_dir(&path, mod_rs);
                krate.modules[pending.module.0 as usize].file = FileId(krate.files.len() as u32);
                self.read_module(krate, pending.module, &path, text, dir);
            }
            Err(warning) => krate.warnings.push(*warning),
        }
    }

    /// Parses `text` (the file at `path`) and reads it into `module`; its `mod name;`
    /// declarations join the queue.
    fn read_module(
        &mut self,
        krate: &mut Crate,
        module: ModuleId,
        path: &Path,
        text: String,
        dir: PathBuf,
    ) {
        let file = FileId(krate.files.len() as u32);
        let shown = path.display().to_string();
        let Some(tree) = self.parser.parse(&text, None) else {
            krate.warnings.push(
                Diagnostic::warning(
                    "rust.autodoc.parse",
                    "RUST SOURCE DOES NOT PARSE",
                    format!("The parser gave up on {shown}; it is left out."),
                    "Check the file with `cargo check`.",
                )
                .in_file(&shown),
            );
            return;
        };
        let root = tree.root_node();
        if let Some(depth_warning) = check_depth(root, self.options.max_depth, &shown, &text) {
            krate.warnings.push(depth_warning);
            return;
        }
        if root.has_error() {
            krate.warnings.push(syntax_warning(root, &shown, &text));
        }
        krate.modules[module.0 as usize].file = file;
        let parsed = ParsedFile { module, file, path, text: &text, root, dir };
        let pending = self.read_tree(krate, parsed);
        self.queue.extend(pending);
        krate.files.push(SourceFile { path: path.to_path_buf(), text });
    }

    /// Reads the items of a parsed file; returns the file modules it declares.
    fn read_tree(&mut self, krate: &mut Crate, parsed: ParsedFile) -> Vec<PendingModule> {
        let ParsedFile { module, file, path, text, root, dir } = parsed;
        let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let file_dir = base.clone();
        let root_dir = krate.root_dir.clone();
        let max_bytes = self.options.max_file_bytes;
        let mut include = move |relative: &str| -> Option<String> {
            read_source(
                &base.join(relative),
                &root_dir,
                &Options { max_file_bytes: max_bytes, ..Options::default() },
            )
            .ok()
        };
        let mut reader = FileReader {
            krate,
            source: text,
            file,
            include: &mut include,
            pending: Vec::new(),
            item_budget: &mut self.item_budget,
            truncated: false,
        };
        reader.read_file(root, module, dir.clone());
        let truncated = reader.truncated;
        let mut pending = std::mem::take(&mut reader.pending);
        // `#[path]` on a file-level `mod` is relative to the file's own folder.
        for p in pending.iter_mut().filter(|p| p.dir == dir) {
            p.path_base = file_dir.clone();
        }
        if truncated && !self.warned_items {
            self.warned_items = true;
            krate.warnings.push(
                Diagnostic::warning(
                    "rust.autodoc.limit",
                    "TOO MANY RUST ITEMS",
                    format!(
                        "The crate declares more than {} items; the rest are left out.",
                        self.options.max_items
                    ),
                    "Raise Options::max_items, or document a smaller crate.",
                )
                .in_file(&path.display().to_string()),
            );
        }
        pending
    }
}

/// Finds the file of a `mod name;`: `#[path]`, then `name.rs`, then `name/mod.rs`.
/// Returns the path and whether it is read as a `mod.rs` file (its children live beside
/// it rather than in a folder named after it).
fn find_module_file(pending: &PendingModule) -> Option<(PathBuf, bool)> {
    if let Some(path) = &pending.path_attr {
        let path = pending.path_base.join(path);
        return path.is_file().then_some((path, true));
    }
    let flat = pending.dir.join(format!("{}.rs", pending.name));
    if flat.is_file() {
        return Some((flat, false));
    }
    let nested = pending.dir.join(&pending.name).join("mod.rs");
    nested.is_file().then_some((nested, true))
}

/// The folder a file module's own `mod` declarations are looked for in.
fn children_dir(path: &Path, mod_rs: bool) -> PathBuf {
    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    if mod_rs {
        return parent;
    }
    match path.file_stem() {
        Some(stem) => parent.join(stem),
        None => parent,
    }
}

fn missing_module(krate: &Crate, pending: &PendingModule) -> Diagnostic {
    let flat = pending.dir.join(format!("{}.rs", pending.name));
    let nested = pending.dir.join(&pending.name).join("mod.rs");
    let message = match &pending.path_attr {
        Some(path) => format!(
            "`mod {}` names the file {path} with #[path], which does not \
            exist.",
            pending.name
        ),
        None => format!(
            "No file holds the module `{}`: neither {} nor {} exists.",
            pending.name,
            flat.display(),
            nested.display()
        ),
    };
    let hint =
        format!("Create {}, or remove the `mod {};` declaration.", flat.display(), pending.name);
    let warning =
        Diagnostic::warning("rust.autodoc.module", "RUST MODULE FILE NOT FOUND", message, &hint);
    let file = pending.file.and_then(|id| krate.files.get(id.0 as usize));
    match file {
        Some(f) => warning.at(&f.path.display().to_string(), &f.text, pending.offset),
        None => warning,
    }
}

/// The canonical form of a path, or the path itself when it cannot be resolved.
fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Reads a source file, refusing files outside `root_dir`, larger than the limit, or not
/// UTF-8.
fn read_source(
    path: &Path,
    root_dir: &Path,
    options: &Options,
) -> Result<String, Box<Diagnostic>> {
    let shown = path.display().to_string();
    let inside = canonical(path).starts_with(canonical(root_dir));
    if !inside {
        return Err(Box::new(
            Diagnostic::warning(
                "rust.autodoc.confined",
                "RUST FILE OUTSIDE THE CRATE",
                format!(
                    "{shown} is outside the crate folder {}, so it is not read.",
                    root_dir.display()
                ),
                "Keep module files and included docs inside the crate folder.",
            )
            .in_file(&shown),
        ));
    }
    let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if size > options.max_file_bytes as u64 {
        return Err(Box::new(
            Diagnostic::warning(
                "rust.autodoc.size",
                "RUST FILE TOO LARGE",
                format!(
                    "{shown} has {size} bytes, more than the limit of {}; it is left out.",
                    options.max_file_bytes
                ),
                "Split the file into modules, or raise Options::max_file_bytes.",
            )
            .in_file(&shown),
        ));
    }
    fs::read_to_string(path).map_err(|e| {
        Box::new(
            Diagnostic::warning(
                "rust.autodoc.read",
                "RUST FILE NOT READABLE",
                format!("{shown} could not be read: {e}."),
                "Check that the file exists, is readable, and is UTF-8.",
            )
            .in_file(&shown),
        )
    })
}

/// A warning when the syntax tree below `root` is deeper than `max_depth`, measured with
/// a cursor walk (no recursion).
fn check_depth(
    root: tree_sitter::Node,
    max_depth: usize,
    file: &str,
    text: &str,
) -> Option<Diagnostic> {
    let mut cursor = root.walk();
    let mut depth = 0usize;
    loop {
        if depth > max_depth {
            let offset = cursor.node().start_byte();
            return Some(
                Diagnostic::warning(
                    "rust.autodoc.nesting",
                    "RUST SOURCE NESTS TOO DEEPLY",
                    format!("The file nests deeper than {max_depth} levels, so it is left out."),
                    "Split the deeply nested expression or module, or raise Options::max_depth.",
                )
                .at(file, text, offset),
            );
        }
        if cursor.goto_first_child() {
            depth += 1;
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return None;
            }
            depth -= 1;
        }
    }
}

/// A warning placed at the first syntax error below `root`.
fn syntax_warning(root: tree_sitter::Node, file: &str, text: &str) -> Diagnostic {
    let mut cursor = root.walk();
    let mut offset = 0;
    loop {
        let node = cursor.node();
        if node.is_error() || node.is_missing() {
            offset = node.start_byte();
            break;
        }
        if node.has_error() && cursor.goto_first_child() {
            continue;
        }
        if !cursor.goto_next_sibling() {
            break;
        }
    }
    Diagnostic::warning(
        "rust.autodoc.parse",
        "RUST SOURCE DOES NOT PARSE",
        "This file has a syntax error; what could be read is documented.".into(),
        "Check the file with `cargo check`.",
    )
    .at(file, text, offset)
}

/// Finds the crate's name, folder and root file from the path the caller gave.
fn locate(path: &Path, options: &Options) -> Result<Located, Error> {
    if path.is_file() {
        let root_dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let fallback = if matches!(stem.as_str(), "lib" | "main" | "mod") {
            folder_name(&root_dir)
        } else {
            stem
        };
        let name = crate_name(options.crate_name.as_deref(), &fallback);
        return Ok(Located {
            name,
            root_dir,
            root_file: Some(path.to_path_buf()),
            loose_files: Vec::new(),
        });
    }
    if !path.is_dir() {
        return Err(Error::new(
            "rust.autodoc.crate",
            "RUST CRATE NOT FOUND",
            format!("{} is neither a folder nor a .rs file.", path.display()),
            "Give the folder holding Cargo.toml, or the crate's lib.rs.",
        ));
    }
    let manifest = path.join("Cargo.toml");
    if manifest.is_file() {
        return locate_package(path, &manifest, options);
    }
    locate_folder(path, options)
}

/// Locates a Cargo package: `[lib] path` or `src/lib.rs`, else `src/main.rs`.
fn locate_package(dir: &Path, manifest: &Path, options: &Options) -> Result<Located, Error> {
    let text = fs::read_to_string(manifest).unwrap_or_default();
    let package_name = manifest_value(&text, "package", "name");
    let lib_name = manifest_value(&text, "lib", "name");
    let fallback = lib_name.or(package_name).unwrap_or_else(|| folder_name(dir));
    let name = crate_name(options.crate_name.as_deref(), &fallback);
    let candidates = [
        manifest_value(&text, "lib", "path").map(|p| dir.join(p)),
        Some(dir.join("src").join("lib.rs")),
        Some(dir.join("src").join("main.rs")),
    ];
    let root_file = candidates.into_iter().flatten().find(|p| p.is_file());
    match root_file {
        Some(root) => Ok(Located {
            name,
            root_dir: dir.to_path_buf(),
            root_file: Some(root),
            loose_files: Vec::new(),
        }),
        None => Err(Error::new(
            "rust.autodoc.root",
            "RUST CRATE ROOT NOT FOUND",
            format!(
                "The package in {} has no src/lib.rs, no src/main.rs, and no [lib] path.",
                dir.display()
            ),
            "Create src/lib.rs, or set `path` under [lib] in Cargo.toml.",
        )),
    }
}

/// Locates a folder without `Cargo.toml`: a root file in it, or its loose `.rs` files.
fn locate_folder(dir: &Path, options: &Options) -> Result<Located, Error> {
    let name = crate_name(options.crate_name.as_deref(), &folder_name(dir));
    for root in ["lib.rs", "main.rs", "mod.rs"] {
        let file = dir.join(root);
        if file.is_file() {
            return Ok(Located {
                name,
                root_dir: dir.to_path_buf(),
                root_file: Some(file),
                loose_files: Vec::new(),
            });
        }
    }
    let mut loose: Vec<PathBuf> = fs::read_dir(dir)
        .map(|entries| entries.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    loose.retain(|p| p.is_file() && p.extension().is_some_and(|e| e == "rs"));
    loose.sort();
    if loose.is_empty() {
        return Err(Error::new(
            "rust.autodoc.root",
            "RUST CRATE ROOT NOT FOUND",
            format!(
                "{} has no Cargo.toml, no lib.rs, main.rs or mod.rs, and no .rs files.",
                dir.display()
            ),
            "Give the folder holding Cargo.toml, or a folder of .rs files.",
        ));
    }
    Ok(Located { name, root_dir: dir.to_path_buf(), root_file: None, loose_files: loose })
}

/// The crate name: the caller's, else the fallback with `-` made `_`, as rustc does.
fn crate_name(given: Option<&str>, fallback: &str) -> String {
    given.unwrap_or(fallback).replace('-', "_")
}

fn folder_name(dir: &Path) -> String {
    canonical(dir).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

/// The string value of `key` in the `[section]` table of a Cargo manifest. Only the
/// plain `key = "value"` form is read: enough for `name` and `path`.
fn manifest_value(text: &str, section: &str, key: &str) -> Option<String> {
    let mut current = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            current = line.trim_matches(['[', ']']).trim().to_string();
            continue;
        }
        if current != section {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let v = v.trim();
        if k.trim() == key && v.starts_with('"') {
            return Some(v.trim_matches('"').to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_values_are_read_per_section() {
        let text = "[package]\nname = \"gd-x\"\n\n[lib]\npath = \"src/x.rs\"\n";
        assert_eq!(manifest_value(text, "package", "name").as_deref(), Some("gd-x"));
        assert_eq!(manifest_value(text, "lib", "path").as_deref(), Some("src/x.rs"));
        assert_eq!(manifest_value(text, "lib", "name"), None);
    }

    #[test]
    fn children_of_flat_files_live_in_a_folder() {
        assert_eq!(children_dir(Path::new("src/a.rs"), false), PathBuf::from("src/a"));
        assert_eq!(children_dir(Path::new("src/a/mod.rs"), true), PathBuf::from("src/a"));
    }
}
