//! Unit tests: each construct read from reStructuredText (or MyST) is written back,
//! and the output reads into the same tree.

use guidedoc::{
    Emitter, ReadOptions, RenderContext, RenderOptions, Reports, Workspace, default_request,
    dump_to_string, read_document, read_extension,
};

use crate::{Settings, render_with};

/// Reads `source` with `reader` ("rst", "sphinx", or "myst") and writes it with
/// `width`; returns the output, the outline read, and the notes reported.
fn write_with(reader: &str, source: &str, width: usize) -> (String, String, Vec<String>) {
    let config = guidedoc_sphinx::Config::default();
    let myst = guidedoc_sphinx::myst_settings(config.clone(), Default::default());
    let mut request = default_request("t.rst", source, reader, "rst");
    request.read = match reader {
        "sphinx" => ReadOptions { extension: Some(read_extension(&config)), ..request.read },
        "myst" => ReadOptions { extension: Some(read_extension(&myst)), ..request.read },
        _ => request.read,
    };
    let descriptor = match reader {
        "sphinx" => guidedoc_sphinx::reader(),
        "myst" => guidedoc_commonmark::MYST_READER,
        _ => guidedoc_rst::reader(),
    };
    let mut ws = Workspace::new();
    let doc = read_document(&mut ws, &descriptor, &request).expect("reads");
    let outline = dump_to_string(doc);
    let mut bytes = Vec::new();
    let options = RenderOptions::default();
    let mut reports = Reports::default();
    reports.clear(64);
    let mut ctx = RenderContext::new(doc, Emitter::new(&mut bytes), &options, &mut reports);
    render_with(&mut ctx, Settings { width }).expect("writes");
    let notes = reports.items().iter().map(|d| d.code.to_owned()).collect();
    (String::from_utf8(bytes).expect("utf-8"), outline, notes)
}

/// The outline with soft breaks as spaces in the text around them.
fn comparable(outline: &str) -> String {
    let lines: Vec<&str> = outline.lines().collect();
    let kind =
        |n: usize| lines.get(n).map_or("?", |l| l.trim_start().split(' ').next().unwrap_or(""));
    let mut out: Vec<String> = Vec::new();
    for line in &lines {
        let indent = line.len() - line.trim_start().len();
        let body = line.trim_start();
        let mut body =
            if body == "soft-break" { "text \" \"".to_owned() } else { body.to_owned() };
        if let Some(at) = body.find(" ->") {
            let digits: String = body[at + 3..].chars().take_while(char::is_ascii_digit).collect();
            if let Ok(n) = digits.parse::<usize>() {
                body.replace_range(at + 3..at + 3 + digits.len(), kind(n));
            }
        }
        let body = body.as_str();
        if let (Some(t), Some(prev)) = (body.strip_prefix("text \""), out.last_mut())
            && prev.len() - prev.trim_start().len() == indent
            && prev.trim_start().starts_with("text \"")
        {
            prev.pop();
            prev.push_str(t.strip_suffix('"').unwrap_or(t));
            prev.push('"');
            continue;
        }
        out.push(format!("{}{body}", " ".repeat(indent)));
    }
    out.join("\n")
}

/// Writes `source` and checks that the output reads back into the same tree; returns
/// the output.
fn round_trip(reader: &str, source: &str) -> String {
    let (out, outline, _) = write_with(reader, source, 92);
    let second = if reader == "myst" { "sphinx" } else { reader };
    let (_, again, _) = write_with(second, &out, 92);
    if reader != "myst" {
        let (a, b) = (comparable(&outline), comparable(&again));
        let i = a.lines().zip(b.lines()).take_while(|(x, y)| x == y).count();
        let line = |s: &str| s.lines().nth(i).unwrap_or("<end>").to_owned();
        assert!(a == b, "line {i}: {} | {}\noutput:\n{out}", line(&a), line(&b));
    }
    out
}

#[test]
fn sections_take_one_adornment_per_level() {
    let out = round_trip("rst", "Title\n#####\n\nA\n***\n\nB\n+++\n\nText.\n\nC\n***\n");
    assert!(out.starts_with("Title\n=====\n\nA\n----\n\nB\n~~~~\n"), "{out}");
}

#[test]
fn inline_markup_and_escapes_survive() {
    let source = "Some *emphasis*, **strong**, ``code``, a\\*star, snake_case, name\\_, \
                  x\\|y, `title`, :sub:`2`, foo\\ *bar*\\ baz, http\\://not.a.link.\n";
    let out = round_trip("rst", source);
    assert!(out.contains("foo\\ *bar*\\ baz"), "{out}");
    assert!(out.contains("http\\://not"), "{out}");
}

#[test]
fn paragraphs_wrap_at_the_width_but_not_inside_markup() {
    let source = "word word word `a long link text <https://example.com/a/b>`_ word word word\n";
    let (out, _, _) = write_with("rst", source, 30);
    assert!(out.lines().any(|l| l.ends_with("`a long link text <https://example.com/a/b>`_")));
    assert!(out.lines().filter(|l| !l.contains('`')).all(|l| l.len() <= 30), "{out}");
}

#[test]
fn lists_keep_their_style_and_start() {
    let source =
        "* a\n* b\n\n3) c\n4) d\n\n(iv) e\n(v) f\n\nB. g\nC. h\n\n- one\n\n  two\n- three\n";
    let out = round_trip("rst", source);
    assert!(out.contains("3) c\n4) d") && out.contains("(iv) e"), "{out}");
}

#[test]
fn definition_field_and_option_lists() {
    let source = "term : classifier\n   Definition.\n\n:Field: Body.\n:Other: Two\n   lines.\n\n\
                  -a, --all=FILE  Show all.\n-v  Verbose.\n";
    let out = round_trip("rst", source);
    assert!(out.contains("term : classifier\n   Definition.") && out.contains("-a, --all=FILE"));
}

#[test]
fn line_blocks_and_quotes() {
    let source = "| one\n|    deeper\n| two\n\nPara.\n\n    Quoted.\n\n    -- Someone\n\n\
                  .. epigraph::\n\n   Wise words.\n";
    let out = round_trip("rst", source);
    assert!(out.contains("-- Someone") && out.contains(".. epigraph::"), "{out}");
}

#[test]
fn literal_and_code_blocks() {
    let source = "Example::\n\n    code here\n\nText ::\n\n  more\n\n.. code-block:: python\n   \
                  :linenos:\n   :caption: A *caption*\n   :name: listing\n\n   x = 1\n";
    let out = round_trip("sphinx", source);
    assert!(out.contains("Example::\n\n   code here"), "{out}");
    assert!(out.contains(":caption: A *caption*") && out.contains(":linenos:"), "{out}");
}

#[test]
fn tables_choose_grid_simple_or_list_table() {
    let source = "=====  =====\nA      B\n=====  =====\n1      2\n=====  =====\n\n\
                  +---+---+\n| a     |\n+---+---+\n| b | c |\n+---+---+\n\n\
                  .. list-table:: Title\n   :header-rows: 1\n\n   * - H\n     - I\n   \
                  * - - x\n       - y\n     - z\n";
    let out = round_trip("rst", source);
    assert!(out.contains("| A   | B   |") && out.contains("| a     |"), "{out}");
    assert!(out.contains(".. list-table:: Title"), "{out}");
}

#[test]
fn directives_admonitions_figures_images() {
    let source = concat!(
        ".. note:: Careful.\n\n.. admonition:: Custom\n\n   Body.\n\n",
        ".. figure:: a.png\n   :alt: Alt\n   :width: 50%\n   :align: center\n\n",
        "   Caption.\n\n   Legend.\n\n",
        ".. image:: b.png\n   :target: https://example.com\n\n.. topic:: T\n\n",
        "   Body.\n\n.. sidebar:: S\n   :subtitle: Sub\n\n   Body.\n\n",
        ".. rubric:: R\n\n.. container:: c\n\n   x\n",
    );
    let out = round_trip("rst", source);
    assert!(out.contains(".. figure:: a.png") && out.contains(":target: https://example.com"));
}

#[test]
fn notes_targets_references_and_substitutions() {
    let source = "See [1]_, [#auto]_, [CIT]_, ref_, `phrase ref`_, `anon`__, \
                  `embedded <https://x.org>`_, `alias <ref_>`_, |sub|, |img|_ and _`inline`.\n\n\
                  .. [1] One.\n.. [#auto] Auto.\n.. [CIT] Citation.\n.. _ref: https://r.org\n\
                  .. _phrase ref: https://p.org\n__ https://a.org\n\n.. |sub| replace:: *text*\n\
                  .. |img| image:: i.png\n.. _img: https://i.org\n\n.. a comment\n   continued\n";
    let out = round_trip("rst", source);
    assert!(out.contains(".. [#auto] Auto.") && out.contains(".. |sub| replace:: *text*"));
}

#[test]
fn raw_and_math() {
    let source = ".. raw:: html\n\n   <b>x</b>\n\nInline :math:`a^2` math.\n\n.. math::\n   \
                  :label: e\n\n   a = b\n";
    round_trip("sphinx", source);
}

#[test]
fn sphinx_constructs() {
    let source = concat!(
        ".. toctree::\n   :maxdepth: 2\n   :caption: Contents\n\n",
        "   intro\n   Guide <guide>\n\n",
        "See :ref:`Label <lbl>`, :doc:`intro`, :py:func:`~a.b`, :kbd:`C-x`, ",
        ":menuselection:`&File --> Open`, :samp:`ls {dir}`, :abbr:`LIFO (last in)`.\n\n",
        ".. glossary::\n\n   term\n      Meaning.\n\n.. index:: single: foo\n\n",
        ".. only:: html\n\n   HTML only.\n\n.. versionadded:: 1.2\n\n",
        "   Explained.\n\n.. hlist::\n   :columns: 2\n\n   * a\n   * b\n",
    );
    let out = round_trip("sphinx", source);
    assert!(out.contains(":py:func:`~a.b`") && out.contains(":menuselection:`&File --> Open`"));
}

#[test]
fn domain_objects_and_info_fields() {
    let source = ".. py:function:: f(a, b=1) -> int\n   :module: m\n\n   Does things.\n\n   \
                  :param int a: The a.\n   :param b: The b.\n   :returns: Something.\n   \
                  :rtype: int\n";
    let out = round_trip("sphinx", source);
    assert!(out.contains(".. py:function:: f(a, b=1) -> int"), "{out}");
    assert!(out.contains(":param int a: The a."), "{out}");
}

#[test]
fn custom_roles_are_declared() {
    let source = concat!(
        ".. role:: red\n   :class: red\n\n",
        ".. role:: python(code)\n   :language: python\n\n",
        "A :red:`warning` and :python:`x = 1`.\n",
    );
    let out = round_trip("rst", source);
    assert!(out.starts_with(".. role:: red\n"), "{out}");
}

#[test]
fn myst_becomes_restructuredtext() {
    let source = concat!(
        "# Title\n\n",
        "See {doc}`intro` and {ref}`a label <lbl>`, [text](https://x.org).\n\n",
        "```{note}\nCareful.\n```\n\n```python\nx = 1\n```\n\n",
        "| a | b |\n|---|---|\n| 1 | 2 |\n",
    );
    let out = round_trip("myst", source);
    assert!(out.contains(":doc:`intro`") && out.contains(":ref:`a label <lbl>`"), "{out}");
    assert!(out.contains(".. note::") && out.contains(".. code-block:: python"), "{out}");
}

#[test]
fn lossy_constructs_are_noted() {
    let (out, _, notes) = write_with("myst", "a  \nb\n", 92);
    assert!(out.contains("a b"), "{out}");
    assert!(notes.iter().any(|c| c == "rst.write.lossy"), "{notes:?}");
}
