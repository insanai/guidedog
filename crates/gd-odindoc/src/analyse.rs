//! Reading a package: every file parsed, its declarations collected for the targets it
//! builds for, and declarations of one name for different targets merged.

use std::collections::HashMap;

use crate::ast::{Else, ForeignBlock, ForeignImport, ImportDecl, Stmt, ValueDecl, When};
use crate::parser::parse_file;
use crate::platform::{Targets, suffix_targets, when_targets};
use crate::project::{Project, clean, dir_of, join};
use crate::tags::{FilePrivacy, tag_targets};
use crate::text::{dedent, normalize, split_lines};
use crate::values::{Source, kind_of};
use crate::{Decl, Import, Kind, Package, Private};

/// What one file's reading adds to its package.
struct Walker<'w, 'f> {
    source: Source<'f>,
    /// The file's base name.
    name: &'w str,
    path: &'w str,
    private: Private,
    decls: &'w mut Vec<Decl>,
    imports: &'w mut Vec<(Option<String>, String)>,
    project: &'w Project,
}

/// Reads and parses every file of the package `id` in `dir`.
pub(crate) fn analyse(p: &mut Project, id: &str, dir: &str) -> Package {
    let mut pkg = Package { id: id.to_string(), dir: dir.to_string(), ..Package::default() };
    let mut decls = Vec::new();
    let mut raw_imports = Vec::new();
    // doc.odin's package comment, and the first other file's.
    let mut docs = (String::new(), String::new());
    for name in p.source_files(dir) {
        let path = join(dir, &name);
        let max = p.limits.max_file_bytes;
        let Some(index) = p.read_file(&path) else {
            continue;
        };
        let text = p.files[index].text.as_str();
        if text.len() > max {
            pkg.too_large.push(path);
            continue;
        }
        let file = parse_file(text);
        if file.too_deep {
            pkg.too_deep.push(path);
            continue;
        }
        pkg.errors += file.errors as usize;
        let Some(package_name) = file.package else {
            continue;
        };
        let (tagged, privacy) = tag_targets(&file, text);
        let targets = tagged.and(suffix_targets(&name));
        if targets.is_empty() {
            continue;
        }
        pkg.files.push(path.clone());
        pkg.targets = pkg.targets.or(targets);
        let source = Source { file: &file, src: text };
        if pkg.name.is_empty() {
            pkg.name = source.slice(package_name.start, package_name.end).to_string();
        }
        let doc = package_comment(&source);
        if name == "doc.odin" && !doc.is_empty() {
            docs.0 = doc;
        } else if docs.1.is_empty() {
            docs.1 = doc;
        }
        let private = match privacy {
            FilePrivacy::Public => Private::Public,
            FilePrivacy::Package => Private::Package,
            FilePrivacy::File => Private::File,
        };
        let mut w = Walker {
            source,
            name: &name,
            path: &path,
            private,
            decls: &mut decls,
            imports: &mut raw_imports,
            project: p,
        };
        w.walk_stmts(&file.decls, targets);
    }
    pkg.doc = if docs.0.is_empty() { docs.1 } else { docs.0 };
    pkg.synopsis = synopsis(&pkg.doc);
    pkg.platform = pkg.targets.display(Targets::SUPPORTED);
    pkg.imports = sorted_imports(raw_imports);
    pkg.decls = merge(decls, pkg.targets);
    pkg
}

/// The comment above a file's package clause, without build tags.
fn package_comment(source: &Source<'_>) -> String {
    let text = source.comment(source.file.package_docs);
    let lines: Vec<&str> = split_lines(&text)
        .filter(|l| {
            let t = l.trim();
            !(t.starts_with('+') && !t.starts_with("+ "))
        })
        .collect();
    dedent(&lines).join("\n")
}

/// The first sentence of a comment's first paragraph, on one line, as plain text:
/// backquotes around code are left out.
pub(crate) fn synopsis(doc: &str) -> String {
    let para = doc.split("\n\n").next().unwrap_or("");
    let plain = para.replace('`', "");
    let one = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    let b = one.as_bytes();
    for i in 0..b.len() {
        if b[i] == b'.' && (i + 1 == b.len() || b[i + 1] == b' ') {
            return one[..=i].to_string();
        }
    }
    one
}

/// Imports by alias and package id, sorted, one per alias. `raw` holds each import's
/// alias as written (if any) and its package id.
fn sorted_imports(raw: Vec<(Option<String>, String)>) -> Vec<Import> {
    let mut imports: Vec<Import> = Vec::new();
    for (alias, id) in raw {
        let alias = match alias {
            Some(a) if !a.is_empty() => a,
            _ => id.rsplit([':', '/']).next().unwrap_or(&id).to_string(),
        };
        if alias != "_" {
            imports.push(Import { alias, id });
        }
    }
    imports.sort_by(|a, b| a.alias.cmp(&b.alias).then_with(|| a.id.cmp(&b.id)));
    imports.dedup_by(|later, kept| later.alias == kept.alias);
    imports
}

impl Walker<'_, '_> {
    fn walk_stmts(&mut self, stmts: &[Stmt], targets: Targets) {
        for stmt in stmts {
            match stmt {
                Stmt::Value(d) => self.value_decl(d, targets, "", &[]),
                Stmt::ForeignBlock(b) => self.foreign_block(b, targets),
                Stmt::ForeignImport(f) => self.foreign_import(f, targets),
                Stmt::Import(i) => self.add_import(i),
                Stmt::When(w) => self.when_stmt(w, targets),
                // Other statements declare nothing to document: #assert and other
                // directives, and what a syntax error leaves.
                Stmt::Other => {}
            }
        }
    }

    fn when_stmt(&mut self, w: &When, targets: Targets) {
        let (mut yes, mut no) = (targets, targets);
        if let Some(cond) = when_targets(self.source.file, self.source.src, w.cond, 0) {
            yes = targets.and(cond);
            no = targets.and(!cond);
        }
        if let Some(body) = &w.body {
            self.walk_stmts(body, yes);
        }
        match w.else_stmt.as_deref() {
            Some(Else::Block(stmts)) => self.walk_stmts(stmts, no),
            Some(Else::When(inner)) => self.when_stmt(inner, no),
            Some(Else::Other) | None => {}
        }
    }

    fn foreign_block(&mut self, b: &ForeignBlock, targets: Targets) {
        let lib = normalize(self.source.slice(b.lib.start, b.lib.end));
        // A foreign block's attributes (link_prefix, default_calling_convention) belong
        // to the block; only its privacy passes to what it declares.
        let shared = self.attribute_texts(&b.attributes);
        for stmt in &b.body {
            if let Stmt::Value(d) = stmt {
                self.value_decl(d, targets, &lib, &shared);
            }
        }
    }

    fn foreign_import(&mut self, f: &ForeignImport, targets: Targets) {
        let Some(name_id) = f.name else {
            return;
        };
        let name = self.source.name(name_id).to_string();
        let paths: Vec<String> = f.paths.iter().map(|&p| self.source.normalized(p)).collect();
        let mut sig = paths.join(", ");
        if paths.len() != 1 {
            sig = format!("{{{sig}}}");
        }
        let attributes = self.attribute_texts(&f.attributes);
        let private = self.private_of(&attributes);
        self.add_decl(Decl {
            signature: format!("{name} {sig}"),
            name,
            kind: Kind::ForeignImport,
            docs: self.source.docs(f.docs, f.comment),
            attributes,
            private,
            targets,
            line: f.line,
            ..Decl::default()
        });
    }

    fn add_import(&mut self, i: &ImportDecl) {
        let quoted = self.source.slice(i.path.start, i.path.end);
        let path = quoted.trim_matches(['"', '`']);
        let mut id = path.to_string();
        if !path.contains(':') {
            let dir = clean(&join(&dir_of(self.path), path));
            if let Some(known) = self.project.dir_package_id(&dir) {
                id = known;
            }
        }
        let alias = i.name.map(|n| self.source.slice(n.start, n.end).to_string());
        self.imports.push((alias, id));
    }

    /// Attributes as written, in source order.
    fn attribute_texts(&self, spans: &[crate::ast::Span]) -> Vec<String> {
        let mut sorted = spans.to_vec();
        sorted.sort_by_key(|s| s.start);
        sorted.iter().map(|s| normalize(self.source.slice(s.start, s.end))).collect()
    }

    /// A declaration's privacy: its `@(private)` attribute, else its file's.
    fn private_of(&self, attributes: &[String]) -> Private {
        let mut level = self.private;
        for a in attributes {
            let body = a.strip_prefix('@').unwrap_or(a);
            let body = body.strip_prefix('(').unwrap_or(body);
            let body = body.strip_suffix(')').unwrap_or(body);
            for item in body.split(',') {
                let (name, value) = match item.trim().split_once('=') {
                    Some((n, v)) => (n, Some(v)),
                    None => (item.trim(), None),
                };
                if name.trim() != "private" {
                    continue;
                }
                let file = value.is_some_and(|v| v.trim().trim_matches('"') == "file");
                level = if file { Private::File } else { level.max(Private::Package) };
            }
        }
        level
    }

    /// Adds each name of a declaration: `a :: 1`, `A :: struct {...}`, `x: int`,
    /// `y := 2`, `N : int : 3`, or `f :: proc(...) {...}`.
    fn value_decl(&mut self, d: &ValueDecl, targets: Targets, lib: &str, shared: &[String]) {
        let attributes = self.attribute_texts(&d.attributes);
        let mut all = shared.to_vec();
        all.extend(attributes.iter().cloned());
        let private = self.private_of(&all);
        let docs = self.source.docs(d.docs, d.comment);
        let ty = d.ty.map(|t| self.source.normalized(t)).unwrap_or_default();
        for (i, &name_id) in d.names.iter().enumerate() {
            if !matches!(self.source.file.node(name_id).kind, crate::ast::Expr::Ident { .. }) {
                continue;
            }
            let name = self.source.name(name_id);
            if name == "_" {
                continue;
            }
            let value = d.values.get(i).copied();
            let mut decl = Decl {
                name: name.to_string(),
                attributes: attributes.clone(),
                docs: docs.clone(),
                foreign_lib: lib.to_string(),
                private,
                targets,
                line: self.source.file.line_of(self.source.file.node(name_id).span.start),
                ..Decl::default()
            };
            if d.mutable {
                decl.kind = Kind::Var;
                decl.signature = self.source.var_signature(name, &ty, value);
            } else {
                decl.kind = kind_of(&self.source, value);
                decl.signature =
                    self.source.const_signature(name, decl.kind, &ty, value, &mut decl.members);
            }
            self.add_decl(decl);
        }
    }

    fn add_decl(&mut self, mut d: Decl) {
        d.file = self.name.to_string();
        d.order = self.decls.len();
        self.decls.push(d);
    }
}

/// Joins the declarations of one name made for different targets: those with the same
/// signature become one, for all their targets; each other signature is a variant of the
/// name. Declarations not made for every target of the package say which.
fn merge(decls: Vec<Decl>, all: Targets) -> Vec<Decl> {
    let mut out: Vec<Decl> = Vec::with_capacity(decls.len());
    let mut index: HashMap<String, Vec<usize>> = HashMap::with_capacity(decls.len());
    for d in decls {
        let seen = index.entry(d.name.clone()).or_default();
        let same = seen
            .iter()
            .copied()
            .find(|&k| out[k].signature == d.signature && out[k].kind == d.kind);
        if let Some(k) = same {
            let o = &mut out[k];
            o.targets = o.targets.or(d.targets);
            if o.docs.is_empty() {
                o.docs = d.docs;
            }
            if o.members.is_empty() {
                o.members = d.members;
            }
            continue;
        }
        let variant = seen.len();
        seen.push(out.len());
        out.push(Decl { variant, ..d });
    }
    for d in &mut out {
        d.availability = d.targets.display(all);
    }
    out
}
