use super::*;
use crate::inline::Link;

/// Resolves nothing: every link stays as written.
struct NoLinks;

impl LinkResolver for NoLinks {
    fn resolve(&mut self, destination: &str, _explicit: bool) -> Option<Link> {
        Some(Link::Unresolved { path: destination.to_string() })
    }
}

fn rst(markdown: &str) -> String {
    let mut out = String::new();
    write_rst(markdown, "", &mut NoLinks, &mut out);
    out
}

#[test]
fn headings_become_rubrics() {
    assert_eq!(
        rst("# Examples\n\nText.\n\nTitle\n-----"),
        ".. rubric:: Examples\n\nText.\n\n.. rubric:: Title\n"
    );
}

#[test]
fn code_fences_pick_the_language_and_hide_lines() {
    let text =
        "```\n# use x;\nlet a = 1;\n## kept\n```\n\n```no_run\nf();\n```\n\n~~~toml\na = 1\n~~~";
    assert_eq!(
        rst(text),
        ".. code-block:: rust\n\n   let a = 1;\n   # kept\n\n\
        .. code-block:: rust\n\n   f();\n\n.. code-block:: toml\n\n   a = 1\n"
    );
}

#[test]
fn code_language_follows_rustdoc() {
    assert_eq!(code_language(""), "rust");
    assert_eq!(code_language("ignore,edition2021"), "rust");
    assert_eq!(code_language("compile_fail,E0308"), "rust");
    assert_eq!(code_language("text"), "text");
    assert_eq!(code_language("sh"), "sh");
}

#[test]
fn nested_lists_and_quotes() {
    let text = "- a\n  - b\n- c\n\n> quote\n> more";
    assert_eq!(rst(text), "- a\n\n  - b\n- c\n\n..\n\n   quote more\n");
}

#[test]
fn ordered_lists_keep_their_start() {
    assert_eq!(rst("3. three\n4. four"), "3. three\n4. four\n");
}

#[test]
fn paragraphs_that_look_like_rest_are_escaped() {
    assert_eq!(rst("- not\n\n+1 vote"), "- not\n\n\\ +1 vote\n");
    assert_eq!(rst("Ends with::"), "Ends with:\\:\n");
    assert_eq!(rst("1\\. Not a list."), "\\ 1. Not a list.\n");
}

#[test]
fn reference_definitions_feed_links() {
    let text = "See [the site].\n\n[the site]: https://example.org";
    assert_eq!(rst(text), "See `the site <https://example.org>`__.\n");
}

#[test]
fn tables_become_list_tables() {
    let text = "| a | b |\n|---|:-:|\n| 1 | 2 \\| 3 |";
    assert_eq!(
        rst(text),
        ".. list-table::\n   :header-rows: 1\n\n   * - a\n     - b\n   \
        * - 1\n     - 2 \\| 3\n"
    );
}

#[test]
fn deep_nesting_is_flattened() {
    let text: String = (0..40).map(|i| format!("{}- x\n", "  ".repeat(i))).collect();
    let out = rst(&text);
    assert!(out.contains("- x"), "{out}");
}

#[test]
fn summaries_are_plain() {
    assert_eq!(summary("Makes a [`Foo`](Foo) *now*.\nMore.\n\nRest."), "Makes a Foo now.");
}
