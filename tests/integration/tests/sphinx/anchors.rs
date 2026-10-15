//! Anchor ids: the ids Sphinx 9.1 publishes for the same source (so pages moved from
//! Sphinx keep their links), and, for every source of the Docutils corpus, the ids of
//! the HTML page are those of the page Docutils 0.22's html5 writer writes (with
//! Sphinx's auto_id_prefix = "id"). tools/rstref --ids stored Docutils' ids in the
//! corpus's ids.txt, kept with guidedoc-rst's conformance data. Like Sphinx, Guidedoc
//! writes no system messages into the page, so their ids are not expected.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use guidedoc::{
    ContentPolicy, NodeId, NodeKind, ReadOptions, Request, Resource, ResourceProvider,
    ResourceStatus, Workspace, build_anchors, default_read_options, read_document, read_extension,
    render_document, trusted_policy,
};
use guidedoc_rst::{Settings, SmartQuotes};
use guidedog_tests::repository_root;

/// The anchors of the nodes `pick` chooses, in document order: each node's ids, then
/// "|" after each when more than one kind is picked.
fn anchors(reader: &str, source: &str, promote_title: bool, pick: &[NodeKind]) -> Vec<String> {
    let registry = guidedoc_defaults::registry();
    let reader = registry.find_reader(reader).unwrap();
    let read = ReadOptions { promote_title, ..default_read_options() };
    let mut request = Request { reader: reader.id, read, ..Request::default() };
    request.source.name = "ids.rst";
    request.source.text = source;
    let mut ws = Workspace::new();
    let doc = read_document(&mut ws, reader, &request).unwrap();
    let anchors = build_anchors(doc, &request.render, true);
    let mut got = Vec::new();
    for (i, node) in doc.nodes().iter().enumerate() {
        if pick.contains(&node.kind) {
            let id = NodeId(u32::try_from(i).unwrap());
            got.extend(anchors.anchor_ids(id).map(str::to_owned));
            if pick.len() > 1 {
                got.push("|".to_owned());
            }
        }
    }
    got
}

#[test]
fn anchors_match_published_sphinx_ids() {
    let source = "Doc\n===\n\nIntro\n-----\n\nx\n\nIntro\n-----\n\ny\n\n\
        1. Numbered Start\n------------------\n\nÜberblick & Straße\n------------------\n\n\
        漢字\n----\n\n.. _My Label:\n\nLabelled\n--------\n\nz\n";
    let got = anchors("rst", source, true, &[NodeKind::Section]);
    assert_eq!(
        got,
        ["intro", "id1", "numbered-start", "uberblick-strasze", "id2", "labelled", "my-label"]
    );
}

/// Read with the Sphinx layer, a document also takes the ids of Sphinx's transforms:
/// AutoNumbering gives captioned figures, titled tables and captioned code blocks without
/// an id the next "idN", and SortIds puts a labelled section's "idN" last (test-numfig).
#[test]
fn sphinx_transforms_give_their_ids() {
    let source = ".. _foo:\n\nFoo\n===\n\n.. figure:: a.png\n\n   cap\n\n\
        .. figure:: b.png\n\n.. _tab:\n\n.. csv-table:: title\n\n   a,b\n\n\
        .. code-block:: python\n   :caption: code\n\n   x\n\n\
        .. code-block:: python\n   :caption: named\n   :name: named\n\n   y\n\n\
        .. _foo_a:\n\nFoo A\n-----\n\nIdentity\n--------\n";
    let pick = [NodeKind::Section, NodeKind::Figure, NodeKind::Table, NodeKind::CodeBlock];
    let got = anchors("sphinx", source, false, &pick);
    assert_eq!(
        got,
        [
            "foo", "id1", "|", "id3", "|", "|", "id4", "tab", "|", "id5", "|", "named", "|",
            "foo-a", "id2", "|", "identity", "|"
        ]
    );
}

/// A title's name holds a citation reference's label and a numbered footnote reference's
/// number, but no auto-numbered footnote's number (Docutils 0.22; test-footnotes).
#[test]
fn title_names_hold_reference_labels() {
    let source = "Doc\n===\n\n.. contents::\n\nA [CIT]_\n--------\n\nA [#]_\n------\n\n\
        B [1]_\n------\n\n.. [CIT] c\n.. [#] n\n.. [1] one\n";
    let registry = guidedoc_defaults::registry();
    let reader = registry.find_reader("rst").unwrap();
    let mut request = Request { reader: "rst", ..Request::default() };
    request.source.text = source;
    let mut ws = Workspace::new();
    let doc = read_document(&mut ws, reader, &request).unwrap();
    let anchors = build_anchors(doc, &request.render, true);
    let got: Vec<&str> = (0..doc.node_count())
        .map(|i| NodeId(u32::try_from(i).unwrap()))
        .filter(|&id| doc.kind(id) == NodeKind::Section)
        .filter_map(|id| anchors.anchor_of(id))
        .collect();
    assert_eq!(got, ["a-cit", "a", "b-1"]);
}

/// Every id AutoNumbering gives is counted: a document of captioned figures needs one id
/// per figure, beyond its title's.
#[test]
fn anchors_have_room_for_every_numbered_item() {
    let source = "Bar\n===\n\n.. figure:: a.png\n\n   one\n\n.. figure:: a.png\n\n   two\n\n\
        .. code-block:: python\n   :caption: three\n\n   x\n";
    let got = anchors("sphinx", source, true, &[NodeKind::Figure]);
    let code = anchors("sphinx", source, true, &[NodeKind::CodeBlock]);
    assert_eq!([got, code].concat(), ["id1", "id2", "id3"]);
}

fn corpus() -> PathBuf {
    repository_root().join("crates/guidedoc-rst/tests/data/docutils")
}

/// Resolves includes against the including source; "<name>" is a standard file.
#[derive(Debug)]
struct CorpusFiles;

impl ResourceProvider for CorpusFiles {
    fn provide(&self, from: &str, name: &str) -> Result<Resource, ResourceStatus> {
        let path = match name.strip_prefix('<').and_then(|n| n.strip_suffix('>')) {
            Some(standard) => corpus().join("include").join(standard),
            None => Path::new(from).parent().unwrap_or(Path::new("")).join(name),
        };
        let text = std::fs::read_to_string(&path).map_err(|_| ResourceStatus::Missing)?;
        Ok(Resource { text: Arc::from(text), canonical: path.to_string_lossy().into_owned() })
    }
}

/// The reader settings a corpus source is read with (see corpus.txt).
fn corpus_settings(names: &[&str], request: &mut Request<'_>) -> Settings {
    let mut settings = Settings::default();
    for &s in names {
        match s {
            "sectsubtitle" => settings.sectsubtitle = true,
            "smartquotes" => settings.smart_quotes = SmartQuotes::On,
            "smartquotes-alt" => settings.smart_quotes = SmartQuotes::Alt,
            "pep" => settings.pep_references = true,
            "rfc" => settings.rfc_references = true,
            "trim" => settings.trim_footnote_reference_space = true,
            "nodocinfo" => settings.docinfo = false,
            "notitle" => request.read.promote_title = false,
            "charlevel" => settings.character_level_inline_markup = true,
            "nofile" => request.policy.includes = false,
            "noraw" => request.policy.raw = ContentPolicy::Refuse,
            s if s.starts_with("lang=") => settings.language = s[5..].to_owned(),
            s if s.starts_with("tab=") => settings.tab_width = s[4..].parse().unwrap_or(0),
            _ => {}
        }
    }
    settings
}

/// The ids of the standalone HTML page Guidedoc writes for a corpus source.
fn page_ids(source: &str, names: &[&str]) -> Result<BTreeSet<String>, String> {
    let path = corpus().join(source);
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let name = path.to_string_lossy().into_owned();
    let mut request =
        Request { reader: "rst", resources: Some(&CorpusFiles), ..Request::default() };
    request.policy = trusted_policy();
    request.policy.keep_documents_with_errors = true;
    request.read.today = "2026-09-29";
    request.render.standalone = true;
    let settings = corpus_settings(names, &mut request);
    request.read.extension = Some(read_extension(&settings));
    request.source.name = &name;
    request.source.text = &text;
    let mut ws = Workspace::new();
    read_document(&mut ws, &guidedoc_rst::reader(), &request)
        .map_err(|f| format!("read: {}", f.code()))?;
    let html = render_document(&mut ws, &guidedoc_html::RENDERER, &request)
        .map_err(|f| format!("render: {}", f.code()))?;
    let mut ids = BTreeSet::new();
    for piece in html.split(" id=\"").skip(1) {
        ids.insert(piece[..piece.find('"').unwrap_or(0)].to_owned());
    }
    Ok(ids)
}

/// Docutils' ids per source from ids.txt, without its system messages' ids.
fn docutils_ids() -> HashMap<String, BTreeSet<String>> {
    const MESSAGES: &str = " (system messages)";
    let text = std::fs::read_to_string(corpus().join("ids.txt")).unwrap();
    let (mut ids, mut messages) = (HashMap::new(), HashMap::new());
    for line in text.lines().filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let (source, list) = line.split_once(':').unwrap();
        let set: BTreeSet<String> = list.split_whitespace().map(str::to_owned).collect();
        match source.strip_suffix(MESSAGES) {
            Some(source) => messages.insert(source.to_owned(), set),
            None => ids.insert(source.to_owned(), set),
        };
    }
    for (source, set) in &mut ids {
        if let Some(unwritten) = messages.get(source) {
            set.retain(|id| !unwritten.contains(id));
        }
    }
    ids
}

#[test]
fn rst_anchor_conformance() {
    let expected = docutils_ids();
    let manifest = std::fs::read_to_string(corpus().join("corpus.txt")).unwrap();
    let mut failures = Vec::new();
    let mut total = 0;
    for line in manifest.lines().filter(|l| !l.starts_with('#')) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let Some((&source, names)) = fields.split_first() else { continue };
        total += 1;
        let want = expected.get(source).cloned().unwrap_or_default();
        match page_ids(source, names) {
            Ok(got) if got == want => {}
            Ok(got) => failures.push(format!(
                "{source}: ids differ from Docutils\n  \
                only Docutils: {:?}\n  only Guidedoc: {:?}",
                want.difference(&got).collect::<Vec<_>>(),
                got.difference(&want).collect::<Vec<_>>()
            )),
            Err(e) => failures.push(format!("{source}: {e}")),
        }
    }
    println!("rst anchors: {total} sources, {} with Docutils' ids", total - failures.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
