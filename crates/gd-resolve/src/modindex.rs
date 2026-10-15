//! The Python module index, py-modindex.html, as Sphinx's `PythonModuleIndex` builds it;
//! gd-site writes the page.

use gd_env::{DocId, ModuleInfo};

use crate::view::BuildView;

/// One row of the module index: a module, or a package heading its submodules.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModindexEntry {
    /// The module's name.
    pub name: String,
    /// 0 a module, 1 a package heading its submodules, 2 a submodule.
    pub group: u8,
    /// The page that declares it; `None` for a package with no page of its own.
    pub doc: Option<DocId>,
    /// Its id on that page.
    pub anchor: String,
    /// `:platform:`.
    pub platform: String,
    /// `:deprecated:`.
    pub deprecated: bool,
    /// `:synopsis:`.
    pub synopsis: String,
}

/// The modules under one letter of the module index.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModindexLetter {
    /// The letter.
    pub letter: String,
    /// Its rows, sorted by name, ignoring case.
    pub entries: Vec<ModindexEntry>,
}

/// The Python module index, by letter, `modindex_common_prefix` left out of names.
pub fn module_index(view: &BuildView<'_>) -> Vec<ModindexLetter> {
    let mut letters: Vec<ModindexLetter> = Vec::new();
    let mut previous = String::new();
    for (doc, m) in python_modules(view) {
        let (mut name, mut stripped) = (m.name.as_str(), "");
        for prefix in &view.config.html.modindex_common_prefix {
            if let Some(rest) = m.name.strip_prefix(prefix.as_str()) {
                (name, stripped) = (rest, prefix);
                break;
            }
        }
        if name.is_empty() {
            (name, stripped) = (stripped, ""); // the prefix was the whole name.
        }
        let letter = name.chars().next().map(|c| c.to_lowercase().collect()).unwrap_or_default();
        let entries = letter_entries(&mut letters, letter);
        let parent = name.split('.').next().unwrap_or(name);
        let mut group = 0;
        if parent != name {
            if previous == parent && !entries.is_empty() {
                if let Some(last) = entries.last_mut() {
                    last.group = 1;
                }
            } else if !previous.starts_with(parent) {
                entries.push(ModindexEntry {
                    name: format!("{stripped}{parent}"),
                    group: 1,
                    ..ModindexEntry::default()
                });
            }
            group = 2;
        }
        entries.push(ModindexEntry {
            name: format!("{stripped}{name}"),
            group,
            doc: Some(doc),
            anchor: m.anchor.clone(),
            platform: m.platform.clone(),
            deprecated: m.deprecated,
            synopsis: m.synopsis.clone(),
        });
        previous = name.to_string();
    }
    letters
}

/// The Python modules the project declares, sorted as Sphinx sorts them: by name,
/// ignoring case.
fn python_modules<'v>(view: &BuildView<'v>) -> Vec<(DocId, &'v ModuleInfo)> {
    let mut out: Vec<(DocId, &ModuleInfo)> = view
        .env
        .iter()
        .flat_map(|(id, d)| d.modules.iter().filter(|m| m.domain == "py").map(move |m| (id, m)))
        .collect();
    out.sort_by_cached_key(|(_, m)| m.name.to_lowercase());
    out
}

/// The rows under `letter`, the letter added in order when it is new.
fn letter_entries(letters: &mut Vec<ModindexLetter>, letter: String) -> &mut Vec<ModindexEntry> {
    let at = match letters.binary_search_by(|l| l.letter.cmp(&letter)) {
        Ok(at) => at,
        Err(at) => {
            letters.insert(at, ModindexLetter { letter, entries: Vec::new() });
            at
        }
    };
    &mut letters[at].entries
}

/// Whether the project has a module index: `html_domain_indices` is on and some
/// document declares a Python module.
pub fn has_module_index(view: &BuildView<'_>) -> bool {
    view.config.html.domain_indices
        && view.env.docs.iter().any(|d| d.modules.iter().any(|m| m.domain == "py"))
}
