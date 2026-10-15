//! Generated API pages: the Odin packages' and the Rust crates' pages, which the build
//! reads as it reads files but which never appear in the source folder.

use gd_conf::Config;
use gd_env::{Dependency, Generated, SourceFile, SourceKind};
use gd_report::Report;

/// The API pages the configuration asks for (`odin_autoapi_*`, `rust_autoapi_*`,
/// `apidoc_modules`, and the stub pages of autosummary directives), as generated
/// documents, sorted by docname. A page whose name a document of `taken`
/// already has is left out with a warning (`odin.autoapi.clash`, `rust.autoapi.clash`).
/// What generating the Rust pages found (`rust.autodoc.*`) is added to `reports`, every
/// build, since the pages are generated every build. Pass the result with `taken` to
/// `gd_env::with_generated`.
pub fn api_sources(
    config: &Config,
    taken: &[SourceFile],
    reports: &mut Vec<Report>,
) -> Vec<SourceFile> {
    let mut out = Vec::new();
    crate::autodoc::napoleon_reports(config, reports);
    let odin = crate::odin::odin_api_sources(config);
    keep_unclaimed(odin, taken, "odin", &mut out, reports);
    let rust = crate::rustdoc::rust_api_sources(config, reports);
    keep_unclaimed(rust, taken, "rust", &mut out, reports);
    // apidoc and autosummary leave out the pages the source folder has, with a note.
    let python = crate::apidoc::apidoc_sources(config, taken, reports)
        .into_iter()
        .chain(crate::autosummary::autosummary_sources(config, taken, reports));
    for page in python {
        if !out.iter().any(|o| o.docname == page.docname) {
            out.push(page);
        }
    }
    out.sort_by(|a, b| a.docname.cmp(&b.docname));
    out
}

/// Moves the pages no document of `taken` (or an earlier page) names into `out`, and
/// warns about the others: the document of the source folder is kept.
fn keep_unclaimed(
    pages: Vec<SourceFile>,
    taken: &[SourceFile],
    language: &str,
    out: &mut Vec<SourceFile>,
    reports: &mut Vec<Report>,
) {
    for page in pages {
        let clash = |s: &SourceFile| s.docname == page.docname;
        if !taken.iter().any(clash) && !out.iter().any(clash) {
            out.push(page);
            continue;
        }
        let message = format!(
            "The source folder has a document named {}, the name of a generated API page; \
             the document is kept.",
            page.docname
        );
        let hint =
            format!("Rename the document, or set {language}_autoapi_root to another folder.");
        let code = format!("{language}.autoapi.clash");
        reports.push(Report {
            path: page.relative,
            ..Report::warning(&code, "API PAGE NOT GENERATED", &message).hint(&hint)
        });
    }
}

/// A generated reStructuredText document named `docname`, made from `deps`.
pub(crate) fn generated_source(
    c: &Config,
    docname: &str,
    text: String,
    deps: Vec<Dependency>,
) -> SourceFile {
    let relative = format!("{docname}.rst");
    SourceFile {
        docname: docname.to_owned(),
        path: c.source_dir.join(&relative),
        relative,
        kind: SourceKind::Rst,
        generated: Some(Generated { text, deps }),
    }
}

/// A page title and its `=` underline, then a blank line.
pub(crate) fn title_block(title: &str) -> String {
    let rule = "=".repeat(title.len().max(3));
    format!("{title}\n{rule}\n\n")
}

/// `s` with the characters reStructuredText reads as markup escaped.
pub(crate) fn rst_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | '*' | '`' | '|' | '_' | '[' | ']' | '<' | '>') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_document_of_the_source_folder_keeps_its_name() {
        let c = Config::new("/p/docs", "/p/docs/_build");
        let taken = [generated_source(&c, "api/index", String::new(), Vec::new())];
        let pages = vec![
            generated_source(&c, "api/index", "x".into(), Vec::new()),
            generated_source(&c, "api/shapes", "y".into(), Vec::new()),
        ];
        let (mut out, mut reports) = (Vec::new(), Vec::new());
        keep_unclaimed(pages, &taken, "odin", &mut out, &mut reports);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].docname, "api/shapes");
        assert_eq!(out[0].relative, "api/shapes.rst");
        assert_eq!(reports[0].code, "odin.autoapi.clash");
        assert_eq!(rst_escape("a_b*c"), "a\\_b\\*c");
    }
}
