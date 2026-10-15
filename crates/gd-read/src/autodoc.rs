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
    /// `autodoc_source_paths`, absolute.
    roots: Vec<PathBuf>,
    files: SourceFiles,
    /// The document's session; `None` until the first directive.
    session: Mutex<Option<Session>>,
}

impl PythonHost {
    /// The host for one document, or `None` when sphinx.ext.autodoc is off.
    pub(crate) fn new(c: &Config) -> Option<PythonHost> {
        if !c.has_extension("sphinx.ext.autodoc") {
            return None;
        }
        let roots = gd_conf::autodoc_roots(c);
        let files = SourceFiles::new(&roots);
        Some(PythonHost { config: autodoc_config(c), roots, files, session: Mutex::new(None) })
    }

    /// Adds the Python files read to `deps`.
    pub(crate) fn dependencies(&self, deps: &mut Vec<Dependency>) {
        self.files.dependencies(MISSING, deps);
    }
}

impl AutodocProvider for PythonHost {
    fn generate(&self, request: &AutodocRequest<'_>) -> AutodocResult {
        let mut out = AutodocResult {
            name: format!("<autodoc {}>", request.argument),
            ..Default::default()
        };
        let mut slot = self.session.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.is_none() {
            let files = self.files.clone();
            let reader = Box::new(move |path: &std::path::Path| files.read(path));
            match gd_pyscan::Project::with_reader(&self.roots, reader) {
                Ok(project) => *slot = Some(Session::new(project)),
                Err(e) => {
                    out.messages.push(static_message(PYTHON_GRAMMAR, Some(&e.to_string())));
                    return out;
                }
            }
        }
        let Some(session) = slot.as_mut() else { return out };
        let generated = gd_autodoc::document(session, &self.config, &python_request(request));
        drop(slot);
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
        ..defaults
    }
}

/// Gives autodoc's warnings Sphinx's category, so `suppress_warnings = ["autodoc"]`
/// silences them as it does in Sphinx.
pub(crate) fn tag_autodoc_reports(reports: &mut [gd_report::Report]) {
    for r in reports.iter_mut().filter(|r| r.code.starts_with("autodoc.")) {
        let category =
            if r.code == "autodoc.import" { "autodoc.import_object" } else { "autodoc" };
        r.category = category.to_owned();
    }
}
