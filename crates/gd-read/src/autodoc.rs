//! sphinx.ext.autodoc. The reader hands each autodoc directive to this
//! host, which finds the object in the Python source below `autodoc_source_paths`
//! (gd-pyscan, never running it) and answers with the reStructuredText autodoc would
//! generate (gd-autodoc). Every Python file read is a dependency of the document, so
//! editing it reads the page again; a document whose object was not found is read again
//! every build, as Sphinx reads it again, in case the source appears.

use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use gd_autodoc::{
    ClassContent, ClassSignature, DefaultOption, DescriptionTarget, DirectiveOption, MemberOrder,
    Request, Session, TypeAlias, Typehints, TypehintsFormat,
};
use gd_conf::Config;
use gd_env::Dependency;
use guidedoc::Severity;
use guidedoc_sphinx::{AutodocProvider, AutodocRequest, AutodocResult};

use crate::files::SourceFiles;
use crate::messages::{NO_AUTODOC_PATHS, PYTHON_GRAMMAR, host_message, static_message};

/// The dependency that stands for "an object was not found".
const MISSING: &str = "<autodoc: an object was not found>";

/// Serves the autodoc directives of one document's read.
///
/// The directives of a document share one autodoc session, made at the first: each
/// module is parsed once per document rather than once per directive, and what autodoc
/// merges into a module as it documents it (annotations from type comments) holds for
/// the directives after, as in Sphinx. The files it reads are read from the disk once
/// per document and recorded here.
#[derive(Debug)]
pub(crate) struct PythonHost {
    config: gd_autodoc::Config,
    /// `autosummary_filename_map`.
    filename_map: std::collections::BTreeMap<String, String>,
    /// `autodoc_source_paths`, absolute.
    roots: Vec<PathBuf>,
    files: SourceFiles,
    /// The document's session; `None` until the first directive.
    session: Mutex<Option<Session>>,
}

impl PythonHost {
    /// The host for one document, or `None` when sphinx.ext.autodoc is off.
    pub(crate) fn new(c: &Config) -> Option<PythonHost> {
        if !uses_autodoc(c) {
            return None;
        }
        let roots = python_roots(c);
        let files = SourceFiles::new(&roots);
        Some(PythonHost {
            config: autodoc_config(c),
            filename_map: c.ext.autosummary.filename_map.clone(),
            roots,
            files,
            session: Mutex::new(None),
        })
    }

    /// Adds the Python files read to `deps`.
    pub(crate) fn dependencies(&self, deps: &mut Vec<Dependency>) {
        self.files.dependencies(MISSING, deps);
    }
}

/// Whether autodoc reads Python: sphinx.ext.autodoc, or an extension that sets it up
/// as Sphinx's do (napoleon, autosummary, apidoc).
pub(crate) fn uses_autodoc(c: &Config) -> bool {
    ["sphinx.ext.autodoc", "sphinx.ext.napoleon", "sphinx.ext.autosummary", "sphinx.ext.apidoc"]
        .iter()
        .any(|e| c.has_extension(e))
}

/// The folders autodoc reads Python from: `autodoc_source_paths`, then the folders
/// holding the packages of `apidoc_modules`.
pub(crate) fn python_roots(c: &Config) -> Vec<PathBuf> {
    let mut roots = gd_conf::autodoc_roots(c);
    for root in crate::apidoc::package_roots(c) {
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    roots
}

impl PythonHost {
    /// Runs `f` with the document's autodoc session, made at the first directive; the
    /// grammar's failure as a message.
    fn with_session<T>(
        &self,
        f: impl FnOnce(&mut Session) -> T,
    ) -> Result<T, guidedoc_sphinx::AutodocMessage> {
        let mut slot = self.session.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.is_none() {
            let files = self.files.clone();
            let reader = Box::new(move |path: &std::path::Path| files.read(path));
            match gd_pyscan::Project::with_reader(&self.roots, reader) {
                Ok(project) => *slot = Some(Session::new(project)),
                Err(e) => return Err(static_message(PYTHON_GRAMMAR, Some(&e.to_string()))),
            }
        }
        match slot.as_mut() {
            Some(session) => Ok(f(session)),
            None => Err(static_message(PYTHON_GRAMMAR, None)),
        }
    }
}

impl AutodocProvider for PythonHost {
    fn generate(&self, request: &AutodocRequest<'_>) -> AutodocResult {
        let mut out = AutodocResult {
            name: format!("<autodoc {}>", request.argument),
            ..Default::default()
        };
        if request.directive == "autosummary" {
            let summary = self.with_session(|session| {
                crate::autosummary::summary_text(
                    session,
                    &self.filename_map,
                    &self.config,
                    request,
                )
            });
            return match summary {
                Ok(summary) => {
                    if summary.messages.iter().any(|m| m.message.code == "autosummary.import") {
                        self.files.note_missing();
                    }
                    summary
                }
                Err(m) => AutodocResult { messages: vec![m], ..out },
            };
        }
        let python = python_request(request);
        let generated = match self.with_session(|s| gd_autodoc::document(s, &self.config, &python))
        {
            Ok(generated) => generated,
            Err(m) => {
                out.messages.push(m);
                return out;
            }
        };
        for m in &generated.messages {
            if m.code == "autodoc.import" {
                self.files.note_missing();
            }
            let severity = match m.severity {
                gd_autodoc::Severity::Note => Severity::Note,
                gd_autodoc::Severity::Warning => Severity::Warning,
                gd_autodoc::Severity::Error => Severity::Error,
            };
            out.messages.push(host_message(severity, &m.code, &m.title, &m.text, &m.hint));
        }
        if !generated.lines.is_empty() {
            out.text = generated.lines.join("\n");
            out.text.push('\n');
        } else if self.roots.is_empty() {
            out.messages.push(static_message(NO_AUTODOC_PATHS, None));
        }
        out
    }
}

/// The reader's request as gd-autodoc takes it.
fn python_request(request: &AutodocRequest<'_>) -> Request {
    Request {
        directive: request.directive.to_owned(),
        argument: request.argument.to_owned(),
        options: request.options.iter().map(|o| DirectiveOption::new(o.name, &o.value)).collect(),
        content: if request.content.is_empty() {
            Vec::new()
        } else {
            request.content.split('\n').map(str::to_owned).collect()
        },
        module: request.module.to_owned(),
        class: request.class.to_owned(),
    }
}

/// The conf.toml settings gd-autodoc reads. gd-conf has checked the settings that take
/// one of a few values; a value it let through falls back to Sphinx's default.
pub(crate) fn autodoc_config(c: &Config) -> gd_autodoc::Config {
    let a = &c.autodoc;
    let defaults = gd_autodoc::Config::default();
    gd_autodoc::Config {
        autoclass_content: ClassContent::parse(&a.autoclass_content)
            .unwrap_or(defaults.autoclass_content),
        autodoc_class_signature: ClassSignature::parse(&a.class_signature)
            .unwrap_or(defaults.autodoc_class_signature),
        autodoc_default_options: a
            .default_options
            .iter()
            .map(|o| DefaultOption { name: o.name.clone(), value: o.value.clone(), flag: o.flag })
            .collect(),
        autodoc_docstring_signature: a.docstring_signature,
        autodoc_inherit_docstrings: a.inherit_docstrings,
        autodoc_member_order: MemberOrder::parse(&a.member_order)
            .unwrap_or(defaults.autodoc_member_order),
        autodoc_mock_imports: a.mock_imports.clone(),
        autodoc_preserve_defaults: a.preserve_defaults,
        autodoc_typehints: Typehints::parse(&a.typehints).unwrap_or(defaults.autodoc_typehints),
        autodoc_typehints_description_target: DescriptionTarget::parse(
            &a.typehints_description_target,
        )
        .unwrap_or(defaults.autodoc_typehints_description_target),
        autodoc_typehints_format: TypehintsFormat::parse(&a.typehints_format)
            .unwrap_or(defaults.autodoc_typehints_format),
        autodoc_type_aliases: a
            .type_aliases
            .iter()
            .map(|(name, target)| TypeAlias { name: name.clone(), target: target.clone() })
            .collect(),
        autodoc_use_type_comments: a.use_type_comments,
        python_display_short_literal_types: a.python_display_short_literal_types,
        strip_signature_backslash: a.strip_signature_backslash,
        napoleon: c.has_extension("sphinx.ext.napoleon").then(|| napoleon_settings(c)),
        ..defaults
    }
}

/// The `napoleon_*` settings as gd-autodoc takes them.
pub(crate) fn napoleon_settings(c: &Config) -> gd_autodoc::Napoleon {
    let n = &c.ext.napoleon;
    gd_autodoc::Napoleon {
        google_docstring: n.google_docstring,
        numpy_docstring: n.numpy_docstring,
        include_init_with_doc: n.include_init_with_doc,
        include_private_with_doc: n.include_private_with_doc,
        include_special_with_doc: n.include_special_with_doc,
        use_admonition_for_examples: n.use_admonition_for_examples,
        use_admonition_for_notes: n.use_admonition_for_notes,
        use_admonition_for_references: n.use_admonition_for_references,
        use_ivar: n.use_ivar,
        use_param: n.use_param,
        use_rtype: n.use_rtype,
        use_keyword: n.use_keyword,
        preprocess_types: n.preprocess_types,
        type_aliases: n.type_aliases.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        custom_sections: n
            .custom_sections
            .iter()
            .map(|s| gd_autodoc::NapoleonSection { name: s.name.clone(), like: s.like.clone() })
            .collect(),
        attr_annotations: n.attr_annotations,
    }
}

/// `napoleon.section` for each `napoleon_custom_sections` entry that behaves like a
/// section napoleon does not know: its docstrings show it as a rubric.
pub(crate) fn napoleon_reports(c: &Config, reports: &mut Vec<gd_report::Report>) {
    if !c.has_extension("sphinx.ext.napoleon") {
        return;
    }
    for like in gd_autodoc::unknown_custom_sections(&napoleon_settings(c)) {
        let message = format!(
            "napoleon_custom_sections has an entry that behaves like {like:?}, which is not \
             a section napoleon knows; it is shown as a rubric and its text."
        );
        let report =
            gd_report::Report::warning("napoleon.section", "UNKNOWN NAPOLEON SECTION", &message)
                .hint(
                    "Name a section such as \"Parameters\", \"Returns\" or \"Notes\", or \
                       use \"params_style\" or \"returns_style\".",
                );
        let category = "napoleon.section".to_owned();
        reports.push(gd_report::Report {
            path: gd_conf::CONFIG_FILE.to_owned(),
            category,
            ..report
        });
    }
}

/// Gives autodoc's warnings Sphinx's category, so `suppress_warnings = ["autodoc"]`
/// silences them as it does in Sphinx; the warnings of napoleon and autosummary are
/// in categories of their own names (`napoleon.section`).
pub(crate) fn tag_autodoc_reports(reports: &mut [gd_report::Report]) {
    for r in reports.iter_mut() {
        if r.code.starts_with("autodoc.") {
            let category =
                if r.code == "autodoc.import" { "autodoc.import_object" } else { "autodoc" };
            r.category = category.to_owned();
        } else if r.code.starts_with("napoleon.") || r.code.starts_with("autosummary.") {
            r.category.clone_from(&r.code);
        }
    }
}
