//! References to labels, documents, terms, objects; the warnings of those that do not
//! resolve; and the answers that verify a page's trace.

use gd_env::{LabelKind, Lookup, LookupKind, NumberedInfo, OutputFormat, TermInfo};

use super::*;
use crate::lookup_answer;

fn project() -> Vec<DocInfo> {
    let mut index = doc("index", "Home");
    index.toctrees.push(toctree(&["guide/intro", "api"]));
    index.labels.push(label("install", "installing", "Installing", LabelKind::Section));
    index.labels.push(label("fig-arch", "fig-arch", "Architecture", LabelKind::Figure));
    index.labels.push(label("euler", "equation-euler", "", LabelKind::Equation));
    index.numbered.push(NumberedInfo {
        kind: LabelKind::Figure,
        anchor: "fig-arch".into(),
        ids: vec!["fig-arch".into()],
        section: Some(0),
        ..NumberedInfo::default()
    });
    index.numbered.push(NumberedInfo {
        kind: LabelKind::Equation,
        anchor: "equation-euler".into(),
        ids: vec!["equation-euler".into()],
        ..NumberedInfo::default()
    });
    index.terms.push(TermInfo { name: "widget".into(), anchor: "term-widget".into() });
    let mut intro = doc("guide/intro", "Intro");
    intro.sections.push(SectionInfo {
        slug: "first-steps".into(),
        ..section("First steps", "first-steps", Some(0))
    });
    let mut api = doc("api", "API");
    api.objects.push(object("py", "module", "pkg"));
    api.objects.push(object("py", "function", "pkg.open"));
    api.objects.push(object("py", "class", "pkg.File"));
    api.objects.push(object("py", "method", "pkg.File.close"));
    api.objects.push(object("std", "cmdoption", "tool --verbose"));
    api.objects.push(object("js", "function", "lib.run"));
    api.objects.push(object("odin", "procedure", "core:fmt.println"));
    vec![index, intro, api]
}

#[test]
fn labels_documents_and_terms_resolve() {
    let f = Fixture::new(project());
    let (r, trace) = f.resolve("guide/intro", "ref", "Install", |_| {});
    let r = r.unwrap();
    assert_eq!(
        (r.doc, r.anchor.as_str(), r.title.as_str()),
        (Some(f.id("index")), "installing", "Installing")
    );
    assert_eq!(trace.lookups(), [Lookup::new(LookupKind::Label, "install")]);
    let (r, _) = f.resolve("index", "doc", "guide/intro", |_| {});
    assert_eq!(r.unwrap().title, "Intro");
    let (r, _) = f.resolve("index", "doc", "guide/intro.md", |q| q.slug = "first-steps".into());
    assert_eq!(r.unwrap().anchor, "first-steps");
    let (r, _) = f.resolve("index", "doc", "guide/intro", |q| q.slug = "nowhere".into());
    assert_eq!(r.unwrap().warning.unwrap().code, "build.xref.anchor");
    assert_eq!(f.target("api", "term", "Widget"), Some(("index".into(), "term-widget".into())));
    assert_eq!(f.target("api", "keyword", "install"), Some(("index".into(), "installing".into())));
    // Equation labels belong to the math roles.
    assert!(f.target("api", "ref", "euler").is_none());
    let (r, _) = f.resolve("api", "eq", "euler", |_| {});
    assert_eq!(r.unwrap().title, "(1)");
}

#[test]
fn generated_pages_have_labels() {
    let f = Fixture::new(project());
    let (r, _) = f.resolve("guide/intro", "ref", "genindex", |_| {});
    let r = r.unwrap();
    assert_eq!((r.url.as_str(), r.title.as_str()), ("../genindex.html", "Index"));
    let (r, _) = f.resolve("guide/intro", "ref", "search", |q| q.format = OutputFormat::Pdf);
    assert!(r.unwrap().no_link);
}

#[test]
fn numref_fills_the_format_or_warns() {
    let mut config = Config::new("/p", "/p/_build");
    let f = Fixture::with_config(config.clone(), project());
    let (r, _) = f.resolve("api", "numref", "fig-arch", |_| {});
    let r = r.unwrap();
    assert!(r.no_link);
    assert_eq!(r.warning.unwrap().category, "ref.numref");
    config.numfig = true;
    let f = Fixture::with_config(config, project());
    let (r, _) = f.resolve("api", "numref", "fig-arch", |_| {});
    assert_eq!(r.unwrap().title, "Fig. 1");
    let (r, _) = f.resolve("api", "numref", "fig-arch", |q| {
        q.explicit = true;
        q.content = "{name} ({number})".into();
    });
    assert_eq!(r.unwrap().title, "Architecture (1)");
    let (r, _) = f.resolve("api", "numref", "fig-arch", |q| {
        q.explicit = true;
        q.content = "Figure %d".into();
    });
    assert_eq!(r.unwrap().warning.unwrap().code, "build.numref");
}

#[test]
fn python_objects_resolve_from_the_innermost_context() {
    let f = Fixture::new(project());
    let (r, _) = f.resolve("index", "py:meth", "close", |q| {
        q.module = "pkg".into();
        q.class = "File".into();
    });
    assert_eq!(r.unwrap().anchor, "py-pkg-File-close");
    assert_eq!(f.target("index", "py:func", "pkg.open").unwrap().1, "py-pkg-open");
    // A leading "." searches by suffix, among the types the role names.
    let (r, trace) = f.resolve("index", "py:meth", ".close", |q| q.refspecific = true);
    assert_eq!(r.unwrap().anchor, "py-pkg-File-close");
    assert_eq!(trace.lookups().last().unwrap().suffix_parts(), Some(("py:", ".close", "py:meth")));
    let (r, _) = f.resolve("index", "py:func", ".close", |q| q.refspecific = true);
    assert!(r.is_err());
    assert!(f.target("index", "py:any", "open").is_some());
}

#[test]
fn options_js_odin_and_any_resolve() {
    let f = Fixture::new(project());
    let (r, _) = f.resolve("index", "option", "--verbose", |q| {
        q.context.push(("std:program".into(), "tool".into()));
    });
    assert_eq!(r.unwrap().anchor, "std-tool --verbose");
    assert!(f.target("index", "option", "tool --verbose=LEVEL").is_some());
    let (r, _) = f.resolve("index", "js:func", "run()", |q| {
        q.context.push(("js:module".into(), "lib".into()));
    });
    assert!(r.is_ok());
    assert!(f.target("index", "odin:proc", "fmt.println").is_some());
    assert!(f.target("index", "any", "install").is_some());
    let (r, _) = f.resolve("index", "any", "File", |q| {
        q.context.push(("py:module".into(), "pkg".into()));
    });
    assert_eq!(r.unwrap().anchor, "py-pkg-File");
}

#[test]
fn unresolved_references_warn_as_sphinx_and_suggest() {
    let mut f = Fixture::new(project());
    let (r, _) = f.resolve("index", "ref", "instal", |_| {});
    let why = r.unwrap_err();
    assert!(why.warn);
    assert_eq!((why.code.as_str(), why.category.as_str()), ("build.xref.unresolved", "ref.ref"));
    assert_eq!(why.suggestion.as_deref(), Some("install"));
    let report = why.report("index.rst");
    assert_eq!(report.hints.last().unwrap(), "Did you mean `install`?");
    assert!(why.to_string().contains("Hint:"));
    // Domain references warn only when nitpicky, and nitpick_ignore silences them.
    let (r, _) = f.resolve("index", "py:func", "pkg.opn", |_| {});
    assert!(!r.unwrap_err().warn);
    f.options.nitpicky = true;
    let (r, _) = f.resolve("index", "py:func", "pkg.opn", |_| {});
    let why = r.unwrap_err();
    assert!(why.warn);
    assert_eq!(why.suggestion.as_deref(), Some("pkg.open"));
    f.config.nitpick_ignore.push(("py:func".into(), "pkg.opn".into()));
    assert!(!f.resolve("index", "py:func", "pkg.opn", |_| {}).0.unwrap_err().warn);
    f.config.nitpick_ignore.push(("std:envvar".into(), "HOME".into()));
    assert!(!f.resolve("index", "envvar", "HOME", |_| {}).0.unwrap_err().warn);
}

#[test]
fn downloads_are_found_in_the_source_folder() {
    let dir = TempDir::new("download");
    dir.put("files/data.csv", b"a,b\n");
    let config = Config::new(dir.0.clone(), dir.0.join("_build"));
    let mut index = doc("index", "Home");
    index.toctrees.push(toctree(&["files/page"]));
    let f = Fixture::with_config(config, vec![index, doc("files/page", "Page")]);
    let (r, trace) = f.resolve("files/page", "download", "data.csv", |_| {});
    let r = r.unwrap();
    let asset = r.asset.unwrap();
    assert!(asset.target.starts_with("_downloads/") && asset.target.ends_with("/data.csv"));
    assert_eq!(r.url, format!("../{}", asset.target));
    assert_eq!(trace.lookups()[0].kind, LookupKind::File);
    assert!(f.resolve("files/page", "download", "nope.csv", |_| {}).0.is_err());
}

#[test]
fn answers_change_exactly_when_the_target_changes() {
    let mut f = Fixture::new(project());
    let (_, trace) = f.resolve("api", "ref", "install", |_| {});
    let (_, suffix) = f.resolve("index", "py:meth", ".close", |q| q.refspecific = true);
    let mut lookups: Vec<Lookup> = trace.into_lookups();
    lookups.extend(suffix.into_lookups());
    lookups.push(Lookup::new(LookupKind::Doc, "guide/intro"));
    lookups.push(Lookup::new(LookupKind::Label, "not-yet"));
    lookups.push(Lookup::new(LookupKind::Modules, ""));
    lookups.push(Lookup::new(LookupKind::Cpp, ""));
    let answers = |f: &Fixture| -> Vec<gd_env::Digest> {
        lookups.iter().map(|l| lookup_answer(&f.view(), "api", l)).collect()
    };
    let before = answers(&f);
    // An unrelated change: another label, another document's title.
    f.rebuild(|docs| {
        docs[0].labels.push(label("other", "other", "Other", LabelKind::Section));
        docs[0].title = "API reference".into();
    });
    assert_eq!(answers(&f), before);
    // The referenced label moves: only its answer changes.
    f.rebuild(|docs| docs[2].labels[0].anchor = "install-now".into());
    let after = answers(&f);
    let changed: Vec<usize> = (0..before.len()).filter(|&i| before[i] != after[i]).collect();
    assert_eq!(changed, [0]);
    // A label that was missing appears; the intro is retitled; the method is renamed.
    f.rebuild(|docs| {
        docs[2].labels.push(label("not-yet", "now", "Now", LabelKind::Section));
        docs[1].title = "Introduction".into();
        docs[0].objects[3].anchor = "close-anchor".into();
    });
    let last = answers(&f);
    let changed: Vec<usize> = (0..after.len()).filter(|&i| after[i] != last[i]).collect();
    assert_eq!(changed, [2, 3, 4]);
}

#[test]
fn page_urls_follow_the_builder() {
    let mut f = Fixture::new(project());
    let (intro, index) = (f.id("guide/intro"), f.id("index"));
    assert_eq!(f.view().page_url(Some(intro), index, "installing"), "../index.html#installing");
    assert_eq!(f.view().page_url(Some(index), index, ""), "#");
    assert_eq!(f.view().page_url(None, intro, ""), "guide/intro.html");
    f.kind = BuilderKind::Dirhtml;
    assert_eq!(f.view().page_url(Some(intro), index, ""), "../../");
    assert_eq!(f.view().page_url(None, intro, "x"), "guide/intro/#x");
    f.kind = BuilderKind::Singlehtml;
    assert_eq!(f.view().page_url(Some(intro), index, "installing"), "#index-installing");
    assert_eq!(f.view().page_anchor(intro, ""), "document-guide-intro");
    f.kind = BuilderKind::Html;
    f.config.html.link_suffix = "/".into();
    assert_eq!(f.view().page_url(Some(index), intro, ""), "guide/intro/");
}

#[test]
fn names_and_urls_are_normalized() {
    assert_eq!(crate::relative_url("a/b/c.html", "a/d.html"), "../d.html");
    assert_eq!(crate::relative_url("index.html", "a/d.html"), "a/d.html");
    assert_eq!(crate::normalize_doc("a/b", "../c/./d"), "a/c/d");
    assert_eq!(crate::doc_target("guide", "/intro.rst"), "intro");
    assert_eq!(crate::file_target("guide", "img.png"), "guide/img.png");
    assert_eq!(crate::generated_page("modindex"), Some("py-modindex.html"));
    assert_eq!(crate::format_number("Fig. %s", "2", ""), Some("Fig. 2".into()));
    assert_eq!(crate::format_number("%s and %s", "2", ""), None);
    assert_eq!(crate::format_number("100%% %s", "2", ""), Some("100% 2".into()));
    assert_eq!(crate::format_number("{name} {number}", "2", ""), None);
    assert_eq!(crate::object_key("std", "cmdoption", "rm -r"), "std:cmdoption:rm -r");
    assert_eq!(crate::object_key("std", "envvar", "HOME"), "std:HOME");
    assert_eq!(crate::normalize_name("  Two\tWords "), "two words");
    assert_eq!(crate::option_key("git  commit", "-m"), "git-commit -m");
    assert!(crate::role_takes("py:meth", "method") && !crate::role_takes("py:func", "method"));
    assert!(crate::role_takes("odin:type", "struct") && crate::role_takes("c:func", "anything"));
}
