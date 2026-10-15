//! Rendering, error and file-confinement tests. Each test makes its own context: plugin
//! loading is cheap enough, and it proves that contexts can come and go in one process.

use super::*;

fn context() -> Context {
    Context::new().expect("context")
}

/// Renders `source` and returns only the output.
fn render(ctx: &Context, source: &str, options: Options<'_>) -> Result<Vec<u8>, Error> {
    ctx.render(source, options).map(|r| r.output)
}

fn render_svg(source: &str, options: Options<'_>) -> String {
    match render(&context(), source, options) {
        Ok(out) => String::from_utf8(out).expect("utf-8 svg"),
        Err(err) => panic!("render failed: {err}"),
    }
}

fn render_error(source: &str) -> Error {
    render(&context(), source, Options::default()).expect_err("an error")
}

#[test]
fn digraph_renders_nodes_and_edges() {
    let svg = render_svg("digraph G { a -> b; b -> c; a -> c }", Options::default());
    assert!(svg.contains("<svg") && svg.contains("</svg>"));
    for name in ["a", "b", "c"] {
        assert!(svg.contains(&format!("<title>{name}</title>")), "missing node {name}");
    }
    assert_eq!(svg.matches(r#"class="node""#).count(), 3);
    assert_eq!(svg.matches(r#"class="edge""#).count(), 3);
    assert!(svg.contains("<title>a&#45;&gt;b</title>"));
}

#[test]
fn undirected_graph_renders() {
    let svg = render_svg("graph { a -- b -- c; c -- a }", Options::default());
    assert_eq!(svg.matches(r#"class="edge""#).count(), 3);
    assert!(svg.contains("<title>a&#45;&#45;b</title>"));
}

#[test]
fn clusters_render() {
    let source = r#"digraph {
        subgraph cluster_front { label="Front"; parse -> check }
        subgraph cluster_back { label="Back"; emit }
        check -> emit
    }"#;
    let svg = render_svg(source, Options::default());
    assert_eq!(svg.matches(r#"class="cluster""#).count(), 2);
    assert!(svg.contains("<title>cluster_front</title>"));
    assert!(svg.contains(">Front</text>"));
    assert_eq!(svg.matches(r#"class="edge""#).count(), 2);
}

#[test]
fn html_labels_render() {
    let source = r#"digraph { t [shape=plain label=<
        <table border="0" cellborder="1"><tr><td><b>Name</b></td><td port="v">Value</td></tr>
        </table>>]; t:v -> u }"#;
    let svg = render_svg(source, Options::default());
    assert!(svg.contains(">Name</text>"));
    assert!(svg.contains(">Value</text>"));
    assert!(svg.contains(r#"font-weight="bold""#));
}

#[test]
fn unicode_labels_render() {
    let svg = render_svg(r#"digraph { "Größe" -> "日本語" [label="→ λ"] }"#, Options::default());
    assert!(svg.contains(">Größe</text>"));
    assert!(svg.contains(">日本語</text>"));
    assert!(svg.contains(">→ λ</text>"));
}

#[test]
fn every_engine_renders() {
    let ctx = context();
    let source = "graph { a -- b -- c -- a; c -- d; subgraph cluster_x { e -- f } }";
    for engine in Engine::ALL {
        let out = render(&ctx, source, Options { engine, ..Options::default() });
        let out = out.unwrap_or_else(|err| panic!("{engine:?} failed: {err}"));
        assert!(String::from_utf8_lossy(&out).contains("<title>d</title>"), "{engine:?}");
    }
}

#[test]
fn defaults_override_like_dot() {
    let ctx = context();
    let source = "digraph { bgcolor=red; node [shape=box]; a -> b; c [shape=circle] }";
    let attribute = |kind, name: &str, value: &str| Attribute {
        kind,
        name: name.to_string(),
        value: value.to_string(),
    };
    let attributes = [
        attribute(AttributeKind::Graph, "bgcolor", "blue"),
        attribute(AttributeKind::Node, "shape", "diamond"),
        attribute(AttributeKind::Edge, "color", "green"),
    ];
    let options = Options { format: Format::Dot, attributes: &attributes, ..Options::default() };
    let text = String::from_utf8(render(&ctx, source, options).expect("render")).expect("utf8");
    assert!(text.contains("bgcolor=blue"));
    assert!(text.contains("shape=diamond"));
    assert!(text.contains("edge [color=green]"));
    assert!(text.contains("shape=circle")); // explicit values still win
    assert!(!text.contains("shape=box"));

    // The defaults must not leak into the next graph rendered in this process.
    let options = Options { format: Format::Dot, ..Options::default() };
    let plain = render(&ctx, "digraph { a -> b }", options).expect("render");
    let plain = String::from_utf8_lossy(&plain);
    assert!(!plain.contains("diamond"));
    assert!(!plain.contains("blue"));
    assert!(plain.contains(r#"label="\N""#));
}

#[test]
fn defaults_apply_to_svg() {
    let attributes = [
        Attribute { kind: AttributeKind::Node, name: "style".into(), value: "filled".into() },
        Attribute { kind: AttributeKind::Node, name: "fillcolor".into(), value: "gold".into() },
    ];
    let options = Options { attributes: &attributes, ..Options::default() };
    let svg = render_svg("digraph { a -> b }", options);
    assert_eq!(svg.matches(r#"fill="gold""#).count(), 2);
}

#[test]
fn one_context_renders_twice() {
    let ctx = context();
    let first = render(&ctx, "digraph stable { x -> y }", Options::default()).expect("first");
    let second = render(&ctx, "digraph stable { x -> y }", Options::default()).expect("second");
    assert_eq!(first, second);
    // A failure in between must not poison later renders.
    assert!(render(&ctx, "digraph stable { x -> }", Options::default()).is_err());
    let third = render(&ctx, "digraph stable { x -> y }", Options::default()).expect("third");
    assert_eq!(third, first);
}

#[test]
fn threads_share_one_context() {
    let ctx = context();
    std::thread::scope(|scope| {
        let jobs: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    (0..5).all(|_| {
                        let options = Options { engine: Engine::Neato, ..Options::default() };
                        render(&ctx, "digraph { a -> b -> c }", options)
                            .is_ok_and(|out| String::from_utf8_lossy(&out).contains("<svg"))
                    })
                })
            })
            .collect();
        for job in jobs {
            assert!(job.join().expect("thread"));
        }
    });
}

#[test]
fn dot_format_and_version() {
    let ctx = context();
    assert!(!ctx.version().is_empty());
    let options = Options { format: Format::Dot, ..Options::default() };
    let out = render(&ctx, "digraph { a -> b }", options).expect("render");
    assert!(String::from_utf8_lossy(&out).contains("pos="));
    let options = Options { format: Format::Png, ..Options::default() };
    match render(&ctx, "digraph { a -> b }", options) {
        Ok(png) => assert!(!STATIC && png.len() > 8 && &png[1..4] == b"PNG"),
        Err(err) => assert_eq!(err.kind, ErrorKind::MissingFormat),
    }
}

#[test]
fn names_round_trip() {
    for engine in Engine::ALL {
        assert_eq!(Engine::from_name(engine.name()), Some(engine));
    }
    assert_eq!(Engine::from_name("dotty"), None);
    assert_eq!(Format::from_name("svg"), Some(Format::Svg));
}

#[test]
fn unicode_labels_are_measured() {
    let ctx = context();
    let options = Options { format: Format::Dot, ..Options::default() };
    let wide = ctx.render(r#"digraph { "日本語テキスト" }"#, options).expect("wide");
    // The static build's text-layout plugin measures non-ASCII text without Graphviz's
    // "no value for width of non-ASCII character" warning.
    if STATIC {
        assert_eq!(wide.warnings, Vec::<String>::new());
    }
    let wide = wide.output;
    let options = Options { format: Format::Dot, ..Options::default() };
    let narrow = render(&ctx, r#"digraph { "abcdefg" }"#, options).expect("narrow");
    assert!(
        node_width(&String::from_utf8_lossy(&wide))
            > node_width(&String::from_utf8_lossy(&narrow))
    );
}

fn node_width(dot: &str) -> f64 {
    let Some(at) = dot.find("width=") else { return 0.0 };
    let rest = &dot[at + "width=".len()..];
    let end = rest.find([',', ']', '\n']).unwrap_or(rest.len());
    rest[..end].trim_matches('"').parse().unwrap_or(0.0)
}

#[test]
fn syntax_error_has_line_and_column() {
    let err = render_error("digraph {\n  a -> b\n  b -> -> c\n}\n");
    assert_eq!(err.kind, ErrorKind::Syntax);
    assert_eq!((err.line, err.column), (3, 8));
    assert_eq!(err.near, "->");
    assert!(err.hint.contains("node on each side"));
}

#[test]
fn syntax_errors_explain_edge_operators() {
    let undirected = render_error("graph {\n a -> b\n}");
    assert_eq!(undirected.line, 2);
    assert!(undirected.hint.contains("`--`"));
    let directed = render_error("digraph { a -- b }");
    assert_eq!(directed.line, 1);
    assert!(directed.hint.contains("`->`"));
}

#[test]
fn unfinished_input_is_explained() {
    let open = render_error("digraph {\n a -> b\n");
    assert_eq!(open.kind, ErrorKind::Syntax);
    assert!(open.hint.contains("`}`"), "{open:?}");
    let quote = render_error("digraph { a -> \"b }\n");
    assert_eq!(quote.kind, ErrorKind::Syntax);
    assert!(quote.message.contains("quoted string"), "{quote:?}");
    assert_eq!(render_error("  \n").kind, ErrorKind::EmptyInput);
}

#[test]
fn bad_html_label_is_located() {
    let err = render_error("digraph {\n  a [label=<<b>x</i>>]\n}");
    assert_eq!(err.kind, ErrorKind::HtmlLabel);
    assert_eq!(err.line, 2);
    assert!(err.message.contains("mismatched tag"));
}

#[test]
fn warnings_are_collected() {
    let rendered = context().render("digraph { a [shape=boxx] }", Options::default());
    let warnings = rendered.expect("render").warnings;
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("boxx"));
}

#[test]
fn format_error_points_at_the_problem() {
    let source = "digraph {\n  a -> b\n  b -> -> c\n}\n";
    let text = format_error(&render_error(source), source, "flow.dot");
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[0].starts_with("-- DOT SYNTAX ERROR [graphdog.syntax] ---"));
    assert!(lines[0].ends_with(" flow.dot:3:8"));
    assert_eq!(lines[0].len(), 80);
    assert_eq!(lines[4], "2|   a -> b");
    assert_eq!(lines[5], "3|   b -> -> c");
    assert_eq!(lines[6], "          ^^");
    assert!(text.contains("\nHint: "));
    // An unknown column underlines the whole line.
    let err = Error { column: 0, ..render_error(source) };
    assert!(format_error(&err, source, "flow.dot").contains("3|   b -> -> c\n   ^^^^^^^^^^^\n"));
}

#[test]
fn display_has_title_message_and_hint() {
    let text = render_error("digraph { a -> }").to_string();
    assert!(text.starts_with("-- DOT SYNTAX ERROR"));
    assert!(text.contains("Hint: "));
}

/// The SVG must also work as a Typst image, since Guidedog's PDF books embed it that way.
/// Skipped when no typst program is installed.
#[test]
fn svg_compiles_in_typst() {
    let probe = std::process::Command::new("typst").arg("--version").output();
    if !probe.is_ok_and(|o| o.status.success()) {
        eprintln!("typst not found; skipping");
        return;
    }
    let source = r#"digraph { label="Ünïcode → 日本"; a -> b; b [label=<<b>bold</b>>] }"#;
    let svg = render_svg(source, Options::default());
    let dir = std::env::temp_dir().join(format!("graphdog-typst-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(dir.join("g.svg"), svg).expect("svg");
    std::fs::write(dir.join("g.typ"), "#image(\"g.svg\", width: 50%)\n").expect("typ");
    let status = std::process::Command::new("typst")
        .arg("compile")
        .arg(dir.join("g.typ"))
        .arg(dir.join("g.pdf"))
        .output()
        .expect("typst runs");
    let made = dir.join("g.pdf").is_file();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(status.status.success(), "{}", String::from_utf8_lossy(&status.stderr));
    assert!(made);
}

const PIXEL: [u8; 70] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
    0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
    0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64,
    0x60, 0xf8, 0x5f, 0x0f, 0x00, 0x02, 0x87, 0x01, 0x80, 0xeb, 0x47, 0xba, 0x92, 0x00, 0x00,
    0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// Every file a graph names reaches the caller before Graphviz opens it: images,
/// shapefiles, `<IMG>` in HTML-like labels, and imagepath; Graphviz then opens the path the
/// caller gave, and a refused name stops the graph before any layout.
#[test]
fn the_caller_decides_every_file_a_graph_names() {
    let dir = std::env::temp_dir().join(format!("graphdog-files-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let pixel = dir.join("pixel.png");
    std::fs::write(&pixel, PIXEL).expect("pixel");
    let pixel = pixel.to_string_lossy().into_owned();
    let mut seen = Vec::new();
    let mut decide = |attribute: &str, name: &str| {
        seen.push(format!("{attribute}={name}"));
        match (attribute, name) {
            (_, "secret.png") => FileDecision::Refuse,
            ("imagepath", _) => FileDecision::Omit,
            _ => FileDecision::Open(pixel.clone()),
        }
    };
    let source = r#"digraph { imagepath="/elsewhere"; a [image="a.png", label=""]; "#.to_string()
        + r#"b [shape=box, shapefile="b.png"]; "#
        + r#"c [label=<<TABLE><TR><TD><IMG SRC="c&amp;.png"/></TD></TR></TABLE>>] }"#;
    let ctx = context();
    let options = Options { files: Some(&mut decide), ..Options::default() };
    let out = render(&ctx, &source, options);
    let options = Options { files: Some(&mut decide), ..Options::default() };
    let refused = render(&ctx, r#"digraph { a [image="secret.png"] }"#, options);
    let _ = std::fs::remove_dir_all(&dir);
    let out = String::from_utf8(out.expect("render")).expect("utf8");
    for name in ["imagepath=/elsewhere", "image=a.png", "shapefile=b.png", "src=c&.png"] {
        assert!(seen.iter().any(|s| s == name), "{name} was not shown: {seen:?}");
    }
    assert!(out.contains(&pixel));
    assert!(!out.contains("a.png"));
    let err = refused.expect_err("refused");
    assert_eq!(err.kind, ErrorKind::FileRefused);
    assert!(err.message.contains("secret.png"));
}

/// A caller's reserve counts the output before render copies it.
#[test]
fn the_caller_can_refuse_the_output() {
    let ctx = context();
    let mut no = |_bytes: usize| false;
    let options = Options { reserve: Some(&mut no), ..Options::default() };
    let err = render(&ctx, "digraph { a -> b }", options).expect_err("refused");
    assert_eq!(err.kind, ErrorKind::OutOfMemory);
}

/// What gvRenderData returns is checked before graphdog reads it.
#[test]
fn render_data_is_checked_at_the_boundary() {
    let buffer = *b"<svg";
    let data = buffer.as_ptr().cast::<c_char>();
    // SAFETY: `buffer` holds 4 bytes and outlives the views.
    unsafe {
        let rendered = rendered_bytes(data, 4).expect("valid");
        assert_eq!(rendered, b"<svg");
        assert_eq!(rendered.as_ptr(), buffer.as_ptr()); // a view: nothing is copied yet
        assert_eq!(rendered_bytes(ptr::null(), 0), Some(&[][..]));
        assert_eq!(rendered_bytes(data, 0), Some(&[][..]));
        assert_eq!(rendered_bytes(ptr::null(), 7), None);
        assert_eq!(rendered_bytes(data, isize::MAX as usize + 1), None);
        assert_eq!(rendered_bytes(ptr::null(), usize::MAX), None);
    }
}

#[test]
fn img_src_is_found_in_any_case() {
    let label = r#"<TABLE><TR><TD><img  Src = 'x.png' /></TD></TR></TABLE>"#;
    let (start, end) = files::next_img_src(label).expect("found");
    assert_eq!(&label[start..end], "x.png");
    assert_eq!(files::next_img_src("<IMG>"), None);
}
