//! The gettext builder (intl_gettext.odin): Sphinx's message templates, one POT file
//! per text domain (`gettext_compact`), with every translatable message of its
//! documents, in docname order.

use std::collections::HashMap;
use std::path::{Component, Path};

use gd_env::{BuilderKind, CachedFile, DocInfo, glob_match};
use gd_host::{Ledger, Problem};
use gd_i18n::{ExtractOptions, PotHeader, Template, domain_for, format_date};
use gd_outputs::{Generation, SITE_UNIT, UnitRecord, pages_not_written};
use gd_report::Report;
use gd_resolve::{BuildView, OutputBuilder};
use guidedoc::Workspace;

use crate::localize::{additional_targets, compact_mode, moment_of};

/// Reads a document again when its cached object is stale (missing, damaged, or
/// another document's), returning its new object. gd-read provides it.
pub type Reread<'a> = dyn Fn(&DocInfo) -> Result<CachedFile, Problem> + Sync + 'a;

/// The `gettext` builder. Every template is one file of the site unit, rewritten only
/// when more than its creation date changed. A document whose object cannot be loaded
/// is reported and fails the build (nothing is published).
///
/// `Default` loads objects from the store only; set `reread` to read a document again
/// when its object is stale, as the other builders do.
#[derive(Clone, Copy, Default)]
pub struct GettextBuilder<'a> {
    /// Reads a document again when its object is stale.
    pub reread: Option<&'a Reread<'a>>,
}

impl std::fmt::Debug for GettextBuilder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GettextBuilder").field("reread", &self.reread.is_some()).finish()
    }
}

impl OutputBuilder for GettextBuilder<'_> {
    fn kind(&self) -> BuilderKind {
        BuilderKind::Gettext
    }

    fn write(
        &self,
        view: &BuildView<'_>,
        generation: &mut Generation,
        reports: &mut Vec<Report>,
    ) -> Result<(), Problem> {
        let out_dir = generation.paths().out_dir.clone();
        let (templates, failed) = self.extract_all(view, &out_dir, reports);
        let mut unit = generation.unit(SITE_UNIT);
        if failed > 0 {
            unit.fail();
            return Err(pages_not_written(failed, &out_dir));
        }
        let c = view.config;
        let now = view.options.now.unwrap_or_else(std::time::SystemTime::now);
        let date = format_date("%Y-%m-%d %H:%M%z", &moment_of(now), "en");
        let header = PotHeader {
            project: &c.project,
            version: &c.version,
            copyright: &c.copyright,
            last_translator: &c.intl.gettext_last_translator,
            language_team: &c.intl.gettext_language_team,
            creation_date: &date,
        };
        for (domain, mut template) in templates {
            template.set_header(&header);
            let text = template.write();
            let rel = format!("{domain}.pot");
            if same_template(&out_dir.join(&rel), &text, view.budget) {
                unit.keep_file(&rel);
            } else {
                unit.emit_text(&rel, &text)?;
            }
        }
        unit.finish(UnitRecord::default());
        Ok(())
    }
}

impl GettextBuilder<'_> {
    /// Extracts the messages of every document read, in docname order, into one
    /// template per domain (in the order domains first occur). Returns the templates
    /// and how many documents failed, each reported.
    fn extract_all(
        &self,
        view: &BuildView<'_>,
        out_dir: &Path,
        reports: &mut Vec<Report>,
    ) -> (Vec<(String, Template)>, usize) {
        let c = view.config;
        let compact = compact_mode(&c.intl.gettext_compact);
        let mut docs: Vec<&DocInfo> = view.env.docs.iter().filter(|d| !d.failed).collect();
        docs.sort_by(|a, b| a.docname.cmp(&b.docname));
        let mut templates: Vec<(String, Template)> = Vec::new();
        let mut index: HashMap<String, usize> = HashMap::new();
        let mut failed = 0;
        for doc in docs {
            let excluded = c.intl.gettext_exclude_patterns.iter().any(|pattern| {
                glob_match(pattern, &doc.relative) || glob_match(pattern, &doc.docname)
            });
            if excluded {
                continue;
            }
            let domain = domain_for(&doc.docname, &compact);
            let at = *index.entry(domain.to_string()).or_insert_with(|| {
                templates.push((domain.to_string(), Template::new(domain)));
                templates.len() - 1
            });
            if let Err(problem) = self.extract_document(view, doc, &mut templates[at].1, out_dir) {
                reports.push(problem.report());
                failed += 1;
            }
        }
        (templates, failed)
    }

    /// Loads a document's object and adds its messages to `template`.
    fn extract_document(
        &self,
        view: &BuildView<'_>,
        doc: &DocInfo,
        template: &mut Template,
        out_dir: &Path,
    ) -> Result<(), Problem> {
        let c = view.config;
        let path = relative_path(out_dir, &c.source_dir.join(&doc.relative));
        let base = out_dir.display().to_string();
        let options = ExtractOptions {
            targets: additional_targets(c),
            location: c.intl.gettext_location,
            uuid: c.intl.gettext_uuid,
            path: &path,
            docname: &doc.docname,
            base: &base,
        };
        let mut ws = Workspace::new();
        let object = doc.object.as_ref().ok_or_else(|| missing_object(doc))?;
        let stale = match view.objects.load_object(&mut ws, object, view.limits) {
            Ok(document) => {
                template.extract(document, &options);
                return Ok(());
            }
            Err(e) if e.stale && self.reread.is_some() => e,
            Err(e) => return Err(e.problem),
        };
        let Some(reread) = self.reread else { return Err(stale.problem) };
        let fresh = reread(doc)?;
        match view.objects.load_object(&mut ws, &fresh, view.limits) {
            Ok(document) => {
                template.extract(document, &options);
                Ok(())
            }
            Err(mut e) => {
                e.problem.hint = "The document was read again, and its new object failed too: \
                                  report this as a Guidedog bug, with the document and the \
                                  output of guidedog --version."
                    .into();
                Err(e.problem)
            }
        }
    }
}

/// A document reading recorded no object for.
fn missing_object(doc: &DocInfo) -> Problem {
    Problem::new(
        "build.object",
        "CACHED DOCUMENT IS MISSING",
        &doc.relative,
        format!("The document {} has no cached object to extract messages from.", doc.docname),
        "Rebuild with guidedog build -E.",
        5,
    )
}

/// Compares the template in place with `text`, line by line, without their
/// POT-Creation-Date lines. The file in place is read within `budget` and dropped; a
/// file that cannot be read differs.
fn same_template(path: &Path, text: &str, budget: &gd_host::Budget) -> bool {
    let ledger = Ledger::new(budget);
    let Ok(old) = gd_host::read_file(path, Some(&ledger), usize::MAX) else { return false };
    let Ok(old) = std::str::from_utf8(&old) else { return false };
    let kept = |line: &&str| !line.starts_with("\"POT-Creation-Date:");
    old.lines().filter(kept).eq(text.lines().filter(kept))
}

/// `path` relative to the folder `base`, with `..` where it climbs out and `/`
/// separators; both are absolute or both relative to the same folder.
pub(crate) fn relative_path(base: &Path, path: &Path) -> String {
    let base: Vec<Component<'_>> = base.components().collect();
    let path: Vec<Component<'_>> = path.components().collect();
    let common = base.iter().zip(&path).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = vec!["..".to_string(); base.len() - common];
    parts.extend(path[common..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned()));
    parts.join("/")
}
