//! Builds that look inside the crate: how often templates are rendered.

#[path = "../tests/common/mod.rs"]
mod common;

use common::{Project, section, toctree};
use gd_env::BuilderKind;
use gd_outputs::{Generation, GenerationOptions, output_paths};

use crate::site::Site;
use crate::templates::Layout;

/// A layout whose functions read the build and the page (toctree, words, plurals,
/// pathto, hasdoc) is rendered exactly once per page.
#[test]
fn each_page_renders_its_template_once() {
    let mut p = Project::new("render-once");
    p.config.templates_path = vec!["_templates".into()];
    p.write(
        "_templates/layout.html",
        "{{ _('Index') }}|{{ ngettext('a', 'b', 2) }}|{{ pathto('intro') }}|\
         {{ hasdoc('intro') }}|{{ toctree(maxdepth=1) }}|{{ body }}",
    );
    let index = p.add("index.rst", "Home", "Home\n====\n\n.. toctree::\n\n   intro\n");
    index.sections = vec![section(1, "Home", "home", None)];
    index.toctrees = vec![toctree(Some(0), 1, &["intro"])];
    p.add("intro.rst", "Intro", "Intro\n=====\n\nText.\n").sections =
        vec![section(1, "Intro", "intro", None)];
    p.resolve();
    let view = p.view(BuilderKind::Html);
    let mut reports = Vec::new();
    let layout = Layout::load(&view, &mut reports).unwrap();
    let site = Site {
        view: &view,
        layout,
        labels: None,
        sources: &[],
        articles: crate::article_context::ArticleIndex::new(&view),
    };
    let paths = output_paths(&p.config, BuilderKind::Html, None).unwrap();
    let options = GenerationOptions { all: false, budget: &p.budget };
    let mut generation = Generation::begin(paths, options, &mut reports).unwrap();
    crate::pages::write_html(&site, &mut generation, &mut reports).unwrap();
    // index.html, intro.html, search.html and genindex.html.
    assert_eq!(site.layout.renders.get(), 4);
    let intro = std::fs::read_to_string(generation.paths().next.join("intro.html")).unwrap();
    assert!(intro.starts_with("Index|b|intro.html|True|<ul><li class=\"toctree-l1 current\">"));
}
