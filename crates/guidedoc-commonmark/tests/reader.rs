//! Tree shapes of the "commonmark" reader (AST.md, CommonMark mapping) and its bounds
//! on hostile input.

mod common;

use std::time::{Duration, Instant};

use common::dump;
use guidedoc::{Limits, Status, Workspace, convert};

#[track_caller]
fn expect_dump(markdown: &str, expected: &str) {
    assert_eq!(dump(markdown), expected, "{markdown:?}");
}

#[test]
fn tree_shapes_follow_the_ast_contract() {
    expect_dump(
        "```py extra &amp;\ncode\n```\n",
        "document\n  code-block \"code\\n\" @language=\"py\" @info=\"py extra &\"\n",
    );
    expect_dump(
        "<me@example.org> <https://x.org/a>\n",
        "document\n  paragraph\n    link link=email dest=\"me@example.org\"\n      \
         text \"me@example.org\"\n    text \" \"\n    link link=external \
         dest=\"https://x.org/a\"\n      text \"https://x.org/a\"\n",
    );
    expect_dump(
        "3) a\n4) b\n",
        "document\n  list +tight list=arabic,),3\n    list-item\n      paragraph\n        \
         text \"a\"\n    list-item\n      paragraph\n        text \"b\"\n",
    );
    expect_dump(
        "[a]: /u\\*x 'T'\n\n[a]\n",
        "document\n  paragraph\n    link link=external dest=\"/u*x\" title=\"T\"\n      \
         text \"a\"\n",
    );
    expect_dump(
        "<div>\n  x  \n\n",
        "document\n  raw-block \"<div>\\n  x  \\n\" @format=\"html\"\n",
    );
    expect_dump(
        "a\u{0}b <span>\n",
        "document\n  paragraph\n    text \"a\u{fffd}b \"\n    raw-inline \"<span>\" \
         @format=\"html\"\n",
    );
    expect_dump(
        "a  \nb\\\nc\nd\n",
        "document\n  paragraph\n    text \"a\"\n    hard-break\n    text \"b\"\n    \
         hard-break\n    text \"c\"\n    soft-break\n    text \"d\"\n",
    );
    expect_dump(
        "Title\r\n===\r\n\r\n***\r",
        "document\n  heading value=1\n    text \"Title\"\n  thematic-break\n",
    );
    expect_dump(
        "&Agr; &blank; &lowast; &lang; &#0; &#x110000; &ngE;\n",
        "document\n  paragraph\n    text \"&Agr; \u{2423} \u{2217} \u{27e8} \u{fffd} \u{fffd} \
         \u{2267}\u{338}\"\n",
    );
    expect_dump("", "document\n");
}

fn run_hostile(markdown: &str, limits: Limits) -> Result<(), String> {
    let registry = common::registry();
    let mut ws = Workspace::new();
    let mut request = common::reference_request(markdown);
    request.limits = limits;
    let start = Instant::now();
    let result = convert(&mut ws, &registry, &request).map(|_| ()).map_err(|f| {
        assert!(matches!(f.status, Status::Limit), "{}: {:?}", f.code(), f.status);
        f.code().to_owned()
    });
    let head: String = markdown.chars().take(12).collect();
    assert!(start.elapsed() < Duration::from_secs(5), "took {:?} for {head:?}", start.elapsed());
    result
}

#[test]
fn hostile_inputs_end_in_results() {
    let limits = Limits::default();
    let deep = run_hostile(&format!("{} x\n", ">".repeat(100_000)), limits);
    assert_eq!(deep, Err("commonmark.nesting".to_owned()));
    let lists = run_hostile(&format!("{}x\n", "- ".repeat(5_000)), limits);
    assert!(lists.is_err());
    let closers = format!("a{}", "](b)".repeat(5_000));
    let images = run_hostile(&format!("{}{closers}", "![".repeat(5_000)), limits);
    assert_eq!(images, Err("core.limit.depth".to_owned()));
    let patterns = [
        "[", "*a_", "a**", "` ``", "<a b=\"", "<!--", "[a](b 'x", "[a](<b", "[a](", "&#x", "\\",
        "  \n", "<a@b", "<x:y", "![a", "[x]: y\n", "[a](()", "<a b c ", "> - ", "_a*", "`", "**_",
        "]", "[a][", "[]: <",
    ];
    for pattern in patterns {
        let _ = run_hostile(&pattern.repeat(40_000), limits);
    }
    let small = Limits { max_work: 10_000, ..limits };
    let work = run_hostile(&"*a ".repeat(50_000), small);
    assert_eq!(work, Err("core.limit.work".to_owned()));
}
