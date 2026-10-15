//! Publishing through the host: a catalog is a Guidedog project, so the plugin asks
//! guidedog to build it, warnings as errors, with the record books as the project's
//! `pdf_documents` setting.

use std::path::PathBuf;

use gd_plugins::CommandContext;

use crate::{Book, BuildRequest, Builder, Problem, Result};

/// A [`Builder`] that has the host build each publication.
pub(crate) struct HostBuilder<'a, 'h> {
    /// The host's context, which builds projects.
    pub ctx: &'a mut CommandContext<'h>,
    /// The Typst program `--typst` names, for the PDF books.
    pub typst: Option<PathBuf>,
}

impl Builder for HostBuilder<'_, '_> {
    fn build(&mut self, r: &BuildRequest) -> Result<PathBuf> {
        let request = project_request(r, self.typst.as_deref());
        self.ctx.build_project(&request).map_err(|p| {
            Problem::new(&*p.code, &p.title, &*p.path, &*p.message, &*p.hint).exit(p.exit)
        })?;
        Ok(request.output)
    }
}

/// The host's build of `r`: the catalog as a project, into [`BuildRequest::output`],
/// strict, with the books and the Typst program as settings.
pub(crate) fn project_request(
    r: &BuildRequest,
    typst: Option<&std::path::Path>,
) -> gd_plugins::BuildRequest {
    let mut overrides = Vec::with_capacity(2);
    if !r.books.is_empty() {
        overrides.push(books_setting(&r.books));
    }
    if let Some(typst) = typst {
        let mut setting = String::from("typst=");
        gd_toml::write_basic_string(&mut setting, &typst.to_string_lossy());
        overrides.push(setting);
    }
    gd_plugins::BuildRequest {
        project: r.dir.clone(),
        output: r.output(),
        builder: r.target.name().to_string(),
        overrides,
        strict: true,
        ..gd_plugins::BuildRequest::default()
    }
}

/// The `pdf_documents` setting of `books`, as TOML for `-D`. Every value is a quoted
/// string, so titles and names are data, never settings.
pub(crate) fn books_setting(books: &[Book]) -> String {
    let mut out = String::from("pdf_documents=[");
    for (i, book) in books.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        let fields = [
            ("root", &book.root),
            ("file", &book.file),
            ("title", &book.title),
            ("author", &book.author),
        ];
        out.push('{');
        for (j, (name, value)) in fields.into_iter().enumerate() {
            if j > 0 {
                out.push_str(", ");
            }
            out.push_str(name);
            out.push('=');
            gd_toml::write_basic_string(&mut out, value);
        }
        out.push('}');
    }
    out.push(']');
    out
}
