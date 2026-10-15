//! References resolved in this project's tables: documents, labels, numbers, terms,
//! downloads, options, keywords, tokens, `any`, declared object types, and the domain
//! objects.

use gd_env::{Asset, Digest, LabelKind, OutputFormat};

use crate::lookups::Look;
use crate::view::{dir_of, file_target, format_number, generated_title, relative_url};
use crate::xref::{Resolved, Unresolved};
use gd_env::{normalize_name, option_key};

impl Look<'_, '_> {
    /// Resolves the query in the project's tables; `None` when nothing matches.
    pub(crate) fn resolve_local(&mut self) -> Option<Resolved> {
        let q = self.query;
        let target = q.target.as_str();
        match q.reftype.as_str() {
            "doc" => self.resolve_doc(),
            "ref" => {
                let name = normalize_name(target);
                match self.label(&name) {
                    // Equation labels belong to the math domain; :ref: does not see them.
                    Some(l) if l.kind != LabelKind::Equation => {
                        Some(Resolved { title: l.title.clone(), ..Look::at(l) })
                    }
                    _ => self.generated_label(&name),
                }
            }
            "numref" => self.resolve_numref(),
            "eq" | "math:numref" => self.resolve_equation(),
            "term" => self.term(&normalize_name(target)).map(Look::at),
            "download" => self.resolve_download(),
            "option" => self.resolve_option(),
            // Sphinx resolves a keyword through the label of the same name, and the
            // link keeps the keyword as written.
            "keyword" => self.label(&normalize_name(target)).map(Look::at),
            "token" => self.find_object("std:token:", target.strip_prefix('~').unwrap_or(target)),
            "any" => self.resolve_any(),
            reftype => self.resolve_domain(reftype),
        }
    }

    fn resolve_domain(&mut self, reftype: &str) -> Option<Resolved> {
        let target = self.query.target.as_str();
        if let Some(t) = self.view.config.declared_object_type(reftype) {
            return self.find_object(&format!("std:{}:", t.directive), target.trim());
        }
        match reftype.split_once(':').map_or("", |(d, _)| d) {
            "odin" => self.resolve_odin(reftype),
            "rust" => self.resolve_rust(reftype),
            "cpp" => self.resolve_cpp(&reftype[4..]),
            _ => self.resolve_object(reftype),
        }
    }

    fn resolve_doc(&mut self) -> Option<Resolved> {
        let view = self.view;
        let q = self.query;
        let base = dir_of(&view.doc(q.from).docname);
        let other = self.doc_target(base, &q.target)?;
        let d = view.doc(other);
        // A Markdown link such as page.md#slug names a heading anchor on that page.
        if !q.slug.is_empty() {
            if let Some(s) = d.sections.iter().find(|s| s.slug == q.slug) {
                let anchor = s.anchor.clone();
                return Some(Resolved {
                    doc: Some(other),
                    anchor,
                    title: s.title.clone(),
                    ..Resolved::default()
                });
            }
            let message = format!(
                "{} has no heading with the anchor #{}, so the link goes to the top of that \
                 page.",
                q.target, q.slug
            );
            self.warning = Some(Unresolved::warning([
                "build.xref.anchor",
                "HEADING ANCHOR NOT FOUND",
                "",
                &message,
                "Check the heading's text, and that myst_heading_anchors covers its level.",
            ]));
        }
        Some(Resolved { doc: Some(other), title: d.title.clone(), ..Resolved::default() })
    }

    /// The labels Sphinx defines for the pages it generates: genindex, modindex and
    /// search, whether or not the pages are written. In the book, which has no such
    /// pages, a reference shows its title.
    fn generated_label(&mut self, name: &str) -> Option<Resolved> {
        let file = crate::view::generated_page(name)?;
        let view = self.view;
        let title = view.ui.gettext(generated_title(name)).into_owned();
        if self.query.format == OutputFormat::Pdf {
            return Some(Resolved { title, no_link: true, ..Resolved::default() });
        }
        let here = view.page_name(&view.doc(self.query.from).docname);
        Some(Resolved { title, url: relative_url(&here, file), ..Resolved::default() })
    }

    fn resolve_numref(&mut self) -> Option<Resolved> {
        let view = self.view;
        let q = self.query;
        let label =
            self.label(&normalize_name(&q.target)).filter(|l| l.kind != LabelKind::Equation)?;
        // Found, but shown as written with a warning: a reference to what has no number.
        let unlinked = Resolved { no_link: true, ..Resolved::default() };
        if label.kind != LabelKind::Section && !view.config.numfig {
            self.warn_numref(
                &format!("numfig is off, so the {} reference shows its text.", q.target),
                "Set numfig = true in conf.toml to number figures, tables, and code.",
            );
            return Some(unlinked);
        }
        let number = if label.kind == LabelKind::Section {
            view.section_number(label.doc, &label.anchor).to_string()
        } else {
            view.item_number(label.doc, &label.anchor)
        };
        if number.is_empty() {
            self.warn_numref(
                &format!("{} has no number, so the reference shows its text.", q.target),
                "Number its document with a :numbered: toctree, or use :ref:.",
            );
            return Some(unlinked);
        }
        let template =
            if q.explicit { q.content.as_str() } else { view.number_template(label.kind) };
        let Some(title) = format_number(template, &number, &label.title) else {
            self.warn_numref(
                &format!(
                    "{template} is not a numfig format: write %s or {{number}} once for the \
                     number, and {{name}} for the caption."
                ),
                "Write, for example, :numref:`Figure %s <label>`.",
            );
            return Some(unlinked);
        };
        Some(Resolved { title, ..Look::at(label) })
    }

    fn warn_numref(&mut self, message: &str, hint: &str) {
        self.warning = Some(Unresolved::warning([
            "build.numref",
            "REFERENCE WITHOUT A NUMBER",
            "ref.numref",
            message,
            hint,
        ]));
    }

    fn resolve_equation(&mut self) -> Option<Resolved> {
        let view = self.view;
        let label = self.label(&normalize_name(&self.query.target))?;
        if label.kind != LabelKind::Equation {
            return None;
        }
        let number = view.item_number(label.doc, &label.anchor);
        let title = view.config.math_eqref_format.replace("{number}", &number);
        Some(Resolved { title, ..Look::at(label) })
    }

    fn resolve_download(&mut self) -> Option<Resolved> {
        let view = self.view;
        let q = self.query;
        if q.target.contains("://") {
            return Some(Resolved { url: q.target.clone(), ..Resolved::default() });
        }
        let docname = &view.doc(q.from).docname;
        let source = file_target(dir_of(docname), &q.target);
        let path = view.config.source_dir.join(&source);
        if !self.file(&path) || !gd_host::confined(view.roots, &path) {
            return None;
        }
        let name = source.rsplit(['/', '\\']).next().unwrap_or(&source);
        let digest = Digest::of_text(&source).to_string();
        let stored = format!("_downloads/{}/{name}", &digest[..12]);
        let url = if q.format == OutputFormat::Pdf {
            let base = view.config.html.baseurl.trim_end_matches('/');
            if base.starts_with("https://") || base.starts_with("http://") {
                format!("{base}/{stored}")
            } else {
                source.clone()
            }
        } else {
            relative_url(&view.page_name(docname), &stored)
        };
        let asset = Some(Asset { source: path, target: stored });
        Some(Resolved { url, asset, ..Resolved::default() })
    }

    /// Finds a command-line option as Sphinx's `_resolve_option_xref` does: in the
    /// current program; then without a value written after it (`-X dev`, `-o=FILE`,
    /// `-o[=FILE]`); then with its leading words as the program (`unittest -c`).
    fn resolve_option(&mut self) -> Option<Resolved> {
        let q = self.query;
        let program = q.context("std:program");
        let option = q.target.trim();
        if let Some(r) = self.find_object("std:cmdoption:", &option_key(program, option)) {
            return Some(r);
        }
        for needle in ["=", "[=", " "] {
            let Some((stem, _)) = option.split_once(needle) else { continue };
            if let Some(r) = self.find_object("std:cmdoption:", &option_key(program, stem)) {
                return Some(r);
            }
        }
        let mut commands: Vec<&str> = Vec::new();
        let mut rest = option;
        while let Some((word, after)) = rest.split_once(' ') {
            commands.push(word);
            rest = after.trim_start();
            let key = option_key(&commands.join(" "), rest);
            if let Some(r) = self.find_object("std:cmdoption:", &key) {
                return Some(r);
            }
        }
        None
    }

    fn resolve_any(&mut self) -> Option<Resolved> {
        let view = self.view;
        let q = self.query;
        if let Some(l) = self.label(&normalize_name(&q.target)) {
            return Some(Resolved { title: l.title.clone(), ..Look::at(l) });
        }
        let base = dir_of(&view.doc(q.from).docname);
        if let Some(other) = self.doc_target(base, &q.target) {
            let title = view.doc(other).title.clone();
            return Some(Resolved { doc: Some(other), title, ..Resolved::default() });
        }
        if let Some(t) = self.term(&normalize_name(&q.target)) {
            return Some(Look::at(t));
        }
        for domain in ["std", "py", "c", "cpp", "js", "rst", "odin", "rust"] {
            let found = match domain {
                "cpp" => self.resolve_cpp_any(),
                "odin" => self.resolve_odin("odin:any"),
                "rust" => self.resolve_rust("rust:any"),
                _ => self.resolve_object(&format!("{domain}:any")),
            };
            if found.is_some() {
                return found;
            }
        }
        None
    }

    /// Looks a domain object up as Sphinx does: from the innermost context
    /// (`module.class.name`, `module.name`) outward, and for a leading `.` by suffix.
    pub(crate) fn resolve_object(&mut self, reftype: &str) -> Option<Resolved> {
        let q = self.query;
        let (domain, role) = reftype.split_once(':').unwrap_or(("std", reftype));
        let name = q.target.trim_start_matches(['~', '.', '!']);
        if domain == "js" {
            return self.resolve_js(role == "any", name);
        }
        let module = q.context(&format!("{domain}:module"));
        let class = q.context(&format!("{domain}:class"));
        let mut candidates = Vec::new();
        if !module.is_empty() && !class.is_empty() {
            candidates.push(format!("{module}.{class}.{name}"));
        }
        if !class.is_empty() {
            candidates.push(format!("{class}.{name}"));
        }
        if !module.is_empty() {
            candidates.push(format!("{module}.{name}"));
        }
        // Sphinx's C lookup starts in the scope the reference is written in, then out.
        let mut scope = if domain == "c" { q.context("c:parent") } else { "" };
        while !scope.is_empty() {
            candidates.push(format!("{scope}.{name}"));
            scope = &scope[..scope.rfind('.').unwrap_or(0)];
        }
        candidates.push(name.to_string());
        for c in candidates {
            if let Some(o) = self.object(&format!("{domain}:{c}")) {
                return Some(Look::at(o));
            }
        }
        // Sphinx's Python domain always searches :any: in "refspecific" mode.
        if q.refspecific || q.target.starts_with('.') || (domain == "py" && role == "any") {
            let found = self.suffix(&format!("{domain}:"), &format!(".{name}"), reftype);
            return found.map(Look::at);
        }
        None
    }

    /// Finds a JavaScript object as `JavaScriptDomain.find_obj` does, from the module
    /// and the object the reference is written in: the most specific name first, or,
    /// for a leading `.` and for `any`, the least specific.
    fn resolve_js(&mut self, any: bool, target: &str) -> Option<Resolved> {
        let q = self.query;
        let name = target.strip_suffix("()").unwrap_or(target);
        let module = q.context("js:module");
        let object = q.context("js:object");
        let mut searches = Vec::new();
        if !module.is_empty() && !object.is_empty() {
            searches.push(format!("{module}.{object}.{name}"));
        }
        if !module.is_empty() {
            searches.push(format!("{module}.{name}"));
        }
        if !object.is_empty() {
            searches.push(format!("{object}.{name}"));
        }
        searches.push(name.to_string());
        if any || q.refspecific {
            searches.reverse();
        }
        searches.iter().find_map(|s| self.object(&format!("js:{s}"))).map(Look::at)
    }

    /// Looks up an object whose written name is its text, as options and tokens are,
    /// so the reference keeps what the author wrote.
    pub(crate) fn find_object(&mut self, kind: &str, name: &str) -> Option<Resolved> {
        self.object(&format!("{kind}{name}")).map(Look::at)
    }
}
