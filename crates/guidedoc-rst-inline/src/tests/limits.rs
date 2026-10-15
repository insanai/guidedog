use super::*;
use guidedoc::Status;

#[test]
fn unicode_brackets_and_quotes() {
    expect_inline("（*a*）", "text \"（\"\nemphasis\n  text \"a\"\ntext \"）\"\n");
    expect_inline("（*）", "text \"（*）\"\n");
    expect_inline("„*“", "text \"„*“\"\n");
    expect_inline("»*«", "text \"»*«\"\n");
}

#[test]
fn escaped_white_space_inside_markup() {
    // An escaped space before the end-string is white space for emphasis ...
    // (the escaped space itself disappears from the text).
    expect_inline("*a\\ *", "text \"*a*\"\n| rst.inline.unterminated");
    // ... but interpreted text may end after it.
    expect_inline("`a\\ `_", "reference link=unresolved name=\"a\"\n  text \"a\"\n");
    expect_inline("a\\_ b", "text \"a_ b\"\n");
    expect_inline("`a\\`b`_", "reference link=unresolved name=\"a`b\"\n  text \"a`b\"\n");
}

#[test]
fn empty_input_emits_nothing() {
    fixture("x", false, |f| {
        assert!(parse_inline(&mut f.ctx, &mut f.state, &[], &mut NoRoleHook).is_ok());
        assert_eq!(f.ctx.b.snap.nodes.len(), 1);
    });
}

#[test]
fn work_limit_is_a_result() {
    let source = "some *text* with `markup`_ and more words";
    fixture(source, false, |f| {
        f.ctx.b.limits.max_work = 8;
        let lines = f.lines_of(source);
        let fail = parse_inline(&mut f.ctx, &mut f.state, &lines, &mut NoRoleHook);
        assert_eq!(fail.map_err(|e| e.status), Err(Status::Limit));
        assert_eq!(f.ctx.reports().primary().map(|d| d.code), Some("core.limit.work"));
    });
}

#[test]
fn text_limit_is_a_result() {
    let source = "a_ b_";
    fixture(source, false, |f| {
        f.para(source);
        f.target_with("a", "https://a.org", "", false);
        f.ctx.b.limits.max_text_bytes = f.ctx.b.text_used();
        f.state.options.smart_quotes = crate::SmartQuotes::On;
        let options = f.state.options.clone();
        let result = resolve(&mut f.ctx, &options);
        assert!(result.is_ok() || result.map_err(|e| e.status) == Err(Status::Capacity));
    });
}

/// Unmatched start-strings and near-miss names must not make recognition quadratic.
#[test]
fn hostile_input_stays_linear() {
    let pieces = [
        "*a ", "**a ", "``a ", "`a ", "_`a ", "|a ", "a-", "x@", "-a@a", "[a-", "[#a-", ":a:",
        ":a:`", "a:", "zz:a/ ", "a.b-", "`a`b", "a_b", "-a-a:", "/a", "(*",
    ];
    for piece in pieces {
        let source = piece.repeat(20_000);
        fixture(&source, false, |f| {
            let lines = f.lines_of(&source);
            let p = f.open(NodeKind::Paragraph, "");
            let start = std::time::Instant::now();
            let result = parse_inline(&mut f.ctx, &mut f.state, &lines, &mut NoRoleHook);
            let elapsed = start.elapsed();
            f.close(p);
            assert!(result.is_ok(), "{piece:?}: {result:?}");
            assert!(elapsed < std::time::Duration::from_secs(4), "{piece:?} took {elapsed:?}");
        });
    }
}

const FULL_SOURCE: &str = concat!(
    "Title\n",
    "*em* **st** ``lit`` :sub:`x` :PEP:`8` :RFC:`1#s` `Site <https://s.org>`_ site_\n",
    "[#]_ [*]_ [1]_ [CIT]_ |sub| |sub|_ anon__ _`inline` https://x.org a@b.org\n",
    "`Alias <site_>`_ :custom:`c` *unterminated\n",
    "one\nsym\nmanual\ncit\ntext\n",
);

/// Every inline construct in one document resolves to the expected outcome.
#[test]
fn every_construct_in_one_document() {
    fixture(FULL_SOURCE, true, |f| {
        f.state.define_role(Role { name: "custom".into(), ..Role::default() });
        let s = f.open(NodeKind::Section, "");
        f.ctx.b.set_value(s, 1);
        let h = f.open(NodeKind::Heading, "");
        f.ctx.b.set_value(h, 1);
        f.inlines("Title");
        f.close(h);
        let body = &FULL_SOURCE[6..FULL_SOURCE.find("one\n").unwrap_or(6)];
        f.para(body.trim_end());
        f.footnote("#", "one");
        f.footnote("*", "sym");
        f.footnote("1", "manual");
        f.citation("CIT", "cit");
        f.definition("sub", "text");
        f.target_with("", "https://a.org", "", true);
        f.target_with("sub", "https://sub.org", "", false);
        f.close(s);
        let out = f.finish();
        assert_eq!(f.codes(), "rst.inline.unterminated");
        assert!(out.contains("title\n    text \"Title\""), "{out}");
    });
}
