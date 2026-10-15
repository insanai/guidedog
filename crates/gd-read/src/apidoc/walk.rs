//! `recurse_tree`: the package folder walked as `sphinx-apidoc` walks it, top down with
//! sorted names, and the text of each page it writes.

use std::path::{Path, PathBuf};

use gd_jinja::{Environment, Value};
use gd_report::Report;

use super::{MAX_FOLDER_DEPTH, MAX_MODULES, Settings, limit};

/// The names of an `__init__` module, longest suffix first.
pub(crate) const INIT_FILES: &[&str] = &["__init__.pyx", "__init__.py"];

/// The suffixes of Python modules: source, Cython, and compiled extensions.
const PY_SUFFIXES: &[&str] = &[".py", ".pyx", ".so", ".pyd"];

/// `is_initpy`.
fn is_initpy(file: &str) -> bool {
    INIT_FILES.contains(&file) || (file.starts_with("__init__.") && file.ends_with(".so"))
}

/// `fnmatch.fnmatchcase`: `*` and `?` match any character, `/` included; `[...]` a set.
pub(crate) fn fnmatch(name: &str, pattern: &str) -> bool {
    let (n, p): (Vec<char>, Vec<char>) = (name.chars().collect(), pattern.chars().collect());
    let (mut i, mut j) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while i < n.len() {
        if j < p.len() && p[j] == '*' {
            star = Some(j);
            mark = i;
            j += 1;
            continue;
        }
        if j < p.len()
            && let Some(len) = match_one(&p[j..], n[i])
        {
            i += 1;
            j += len;
            continue;
        }
        match star {
            Some(s) => {
                j = s + 1;
                mark += 1;
                i = mark;
            }
            None => return false,
        }
    }
    p[j..].iter().all(|&c| c == '*')
}

/// Whether the pattern item at the start of `p` matches `c`: its length when it does.
fn match_one(p: &[char], c: char) -> Option<usize> {
    match p[0] {
        '?' => Some(1),
        '[' => {
            let close = p.iter().skip(2).position(|&x| x == ']').map(|k| k + 2)?;
            let body = &p[1..close];
            let (negate, body) = match body.first() {
                Some('!') => (true, &body[1..]),
                _ => (false, body),
            };
            let mut hit = false;
            let mut k = 0;
            while k < body.len() {
                if k + 2 < body.len() && body[k + 1] == '-' {
                    hit |= body[k] <= c && c <= body[k + 2];
                    k += 3;
                } else {
                    hit |= body[k] == c;
                    k += 1;
                }
            }
            (hit != negate).then_some(close + 1)
        }
        x => (x == c).then_some(1),
    }
}

/// `is_excluded`.
fn is_excluded(path: &Path, s: &Settings) -> bool {
    let text = path.to_string_lossy();
    s.excludes.iter().any(|e| fnmatch(&text, e))
}

/// The module files and folders of one folder, as `walk` lists them: sorted, without
/// excluded paths, hidden folders, and (without `include_private`) private ones.
fn list(dir: &Path, s: &Settings) -> (Vec<String>, Vec<String>) {
    let (mut files, mut subs) = (Vec::new(), Vec::new());
    let Ok(entries) = std::fs::read_dir(dir) else { return (files, subs) };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = dir.join(&name);
        let is_link = entry.file_type().is_ok_and(|t| t.is_symlink());
        if path.is_dir() {
            let hidden = name.starts_with('.') || (!s.include_private && name.starts_with('_'));
            if !hidden && !is_excluded(&path, s) && (s.follow_links || !is_link) {
                subs.push(name);
            }
        } else if PY_SUFFIXES.iter().any(|x| name.ends_with(x)) && !is_excluded(&path, s) {
            files.push(name);
        }
    }
    files.sort();
    subs.sort();
    (files, subs)
}

/// `is_skipped_package`.
fn is_skipped_package(dir: &Path, s: &Settings) -> bool {
    if !dir.is_dir() {
        return false;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return true };
    let files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "py"))
        .collect();
    let regular =
        files.iter().any(|f| f.file_name().is_some_and(|n| is_initpy(&n.to_string_lossy())));
    if !regular && !s.implicit_namespaces {
        return true;
    }
    files.iter().all(|f| is_excluded(f, s))
}

/// `is_skipped_module`.
fn is_skipped_module(path: &Path, s: &Settings) -> bool {
    let private = path.file_name().is_some_and(|n| n.to_string_lossy().starts_with('_'));
    !path.exists() || (private && !s.include_private)
}

/// `module_join`.
fn module_join(parts: &[&str]) -> String {
    parts.iter().filter(|p| !p.is_empty()).copied().collect::<Vec<_>>().join(".")
}

/// The pages of a walk: their names (relative to the destination) and texts.
struct Walk<'a> {
    s: &'a Settings,
    env: &'a Environment,
    pages: Vec<(String, String)>,
    toplevels: Vec<String>,
    modules: usize,
    stopped: Option<String>,
}

/// The pages of one `apidoc_modules` entry, the table of contents `modules` last.
pub(crate) fn package_pages(
    s: &Settings,
    env: &Environment,
    reports: &mut Vec<Report>,
) -> Vec<(String, String)> {
    let root_package = if super::is_package_dir(&s.path) || s.implicit_namespaces {
        s.path.file_name().map(|n| n.to_string_lossy().into_owned())
    } else {
        None
    };
    let mut w =
        Walk { s, env, pages: Vec::new(), toplevels: Vec::new(), modules: 0, stopped: None };
    w.visit(&s.path, "", root_package.as_deref(), 0);
    if let Some(what) = w.stopped.take() {
        reports.push(limit(&s.path, &what));
    }
    let toc = w.toc();
    w.pages.push(("modules".to_owned(), toc));
    w.pages
}

impl Walk<'_> {
    /// One folder of the walk, then its subfolders.
    fn visit(&mut self, dir: &Path, subroot: &str, root_package: Option<&str>, depth: usize) {
        if self.stopped.is_some() {
            return;
        }
        if depth > MAX_FOLDER_DEPTH {
            self.stopped = Some(format!("folders more than {MAX_FOLDER_DEPTH} levels deep"));
            return;
        }
        let (mut files, subs) = list(dir, self.s);
        let is_pkg = files.iter().any(|f| is_initpy(f));
        let is_namespace = !is_pkg && self.s.implicit_namespaces;
        if is_pkg {
            files.sort_by_key(|f| !is_initpy(f));
        } else if depth > 0 && !self.s.implicit_namespaces {
            return;
        }
        if is_pkg || is_namespace {
            let worth = !subs.is_empty() || files.len() > 1 || !is_skipped_package(dir, self.s);
            if worth && (!is_namespace || self.has_child_module(dir, 0)) {
                self.package_page(dir, root_package, subroot, &files, &subs, is_namespace);
                self.toplevels.push(module_join(&[root_package.unwrap_or(""), subroot]));
            }
        } else {
            for file in &files {
                if is_skipped_module(&dir.join(file), self.s) {
                    continue;
                }
                let module = file.split('.').next().unwrap_or(file).to_owned();
                self.module_page(&module, &module);
                self.toplevels.push(module);
            }
        }
        for sub in subs {
            let next = if subroot.is_empty() { sub.clone() } else { format!("{subroot}.{sub}") };
            self.visit(&dir.join(&sub), &next, root_package, depth + 1);
        }
    }

    /// `has_child_module`: some folder below `dir` holds a module.
    fn has_child_module(&self, dir: &Path, depth: usize) -> bool {
        let (files, subs) = list(dir, self.s);
        !files.is_empty()
            || (depth < MAX_FOLDER_DEPTH
                && subs.iter().any(|sub| self.has_child_module(&dir.join(sub), depth + 1)))
    }

    /// Counts a module; past the limit the walk stops.
    fn count(&mut self) -> bool {
        self.modules += 1;
        if self.modules > MAX_MODULES {
            self.stopped.get_or_insert_with(|| format!("{MAX_MODULES} modules"));
            return false;
        }
        true
    }

    /// `create_module_file`: the page `qualname`, headed `basename module`.
    fn module_page(&mut self, basename: &str, qualname: &str) {
        if !self.count() {
            return;
        }
        let mut options = self.s.automodule_options.clone();
        if self.s.include_private {
            options.push("private-members".to_owned());
        }
        options.sort();
        options.dedup();
        let ctx = Value::dict();
        ctx.set("show_headings", Value::from(!self.s.no_headings));
        ctx.set("basename", Value::from(basename));
        ctx.set("qualname", Value::from(qualname));
        ctx.set("automodule_options", strings(&options));
        let text = self.render("module.rst.jinja", &ctx);
        self.pages.push((qualname.to_owned(), text));
    }

    /// `create_package_file`: the page of a package, and with `separate_modules` a page
    /// per submodule.
    fn package_page(
        &mut self,
        dir: &Path,
        root_package: Option<&str>,
        subroot: &str,
        files: &[String],
        subs: &[String],
        is_namespace: bool,
    ) {
        if !self.count() {
            return;
        }
        let root = root_package.unwrap_or("");
        let subpackages: Vec<String> = subs
            .iter()
            .filter(|sub| !is_skipped_package(&dir.join(sub), self.s))
            .map(|sub| module_join(&[root, subroot, sub]))
            .collect();
        let mut names: Vec<String> = files
            .iter()
            .filter(|f| !is_skipped_module(&dir.join(f), self.s) && !is_initpy(f))
            .map(|f| f.split('.').next().unwrap_or(f).to_owned())
            .collect();
        names.sort();
        names.dedup();
        let submodules: Vec<String> =
            names.iter().map(|m| module_join(&[root, subroot, m])).collect();
        // The package page's directives take sphinx-apidoc's fixed options.
        let mut options: Vec<String> =
            ["members", "show-inheritance", "undoc-members"].map(String::from).to_vec();
        if self.s.include_private {
            options.push("private-members".to_owned());
            options.sort();
        }
        let pkgname = module_join(&[root, subroot]);
        let ctx = Value::dict();
        ctx.set("pkgname", Value::from(pkgname.as_str()));
        ctx.set("subpackages", strings(&subpackages));
        ctx.set("submodules", strings(&submodules));
        ctx.set("is_namespace", Value::from(is_namespace));
        ctx.set("modulefirst", Value::from(self.s.module_first));
        ctx.set("separatemodules", Value::from(self.s.separate_modules));
        ctx.set("automodule_options", strings(&options));
        ctx.set("show_headings", Value::from(!self.s.no_headings));
        ctx.set("maxdepth", Value::from(self.s.max_depth));
        let text = self.render("package.rst.jinja", &ctx);
        self.pages.push((pkgname, text));
        if self.s.separate_modules {
            for submodule in &submodules {
                self.module_page(submodule, submodule);
            }
        }
    }

    /// `create_modules_toc_file`: the top-level modules and packages, without those
    /// below another.
    fn toc(&self) -> String {
        let mut modules = self.toplevels.clone();
        modules.sort();
        let mut kept: Vec<String> = Vec::new();
        let mut previous = String::new();
        for m in modules {
            if m.starts_with(&format!("{previous}.")) {
                continue;
            }
            previous.clone_from(&m);
            kept.push(m);
        }
        let header =
            self.s.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let ctx = Value::dict();
        ctx.set("header", Value::from(header.as_str()));
        ctx.set("maxdepth", Value::from(self.s.max_depth));
        ctx.set("docnames", strings(&kept));
        self.render("toc.rst.jinja", &ctx)
    }

    fn render(&self, name: &str, ctx: &Value) -> String {
        match self.env.load_template(name).and_then(|t| t.render(ctx)) {
            Ok(text) => text,
            // The templates are Guidedog's own; a failure is a defect, shown on the page.
            Err(e) => format!(".. error::\n\n   {e}\n"),
        }
    }
}

fn strings(items: &[String]) -> Value {
    Value::list(items.iter().map(|s| Value::from(s.as_str())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnmatch_matches_as_python_does() {
        assert!(fnmatch("/p/src/exclude_package.py", "/p/src/exclude_package.py"));
        assert!(fnmatch("/p/src/a/b/test_x.py", "/p/src/*test_*"));
        assert!(fnmatch("x1", "x[0-9]") && !fnmatch("xa", "x[!a-z]") && fnmatch("ab", "a?"));
        assert!(!fnmatch("/p/src/keep.py", "/p/src/exclude*"));
    }
}
