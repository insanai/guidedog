use super::*;

#[test]
fn plain_lines_match_the_placeholder_shape() {
    expect_inline("one line", "text \"one line\"\n");
    expect_inline(
        "first\nsecond\nthird",
        "text \"first\"\nsoft-break\ntext \"second\"\nsoft-break\ntext \"third\"\n",
    );
}

#[test]
fn emphasis_and_strong() {
    expect_inline(
        "an *emphasized* word",
        "text \"an \"\nemphasis\n  text \"emphasized\"\ntext \" word\"\n",
    );
    expect_inline("**bold**", "strong\n  text \"bold\"\n");
    expect_inline("*a\nb*", "emphasis\n  text \"a\"\n  soft-break\n  text \"b\"\n");
    // Content is plain text: markup inside emphasis is not recognized.
    expect_inline("*a ``b`` c*", "emphasis\n  text \"a ``b`` c\"\n");
    expect_inline("*a*b*", "emphasis\n  text \"a*b\"\n");
}

#[test]
fn recognition_rules_reject_markup_inside_words() {
    expect_inline("2*x*3", "text \"2*x*3\"\n");
    expect_inline("2 * x * 3", "text \"2 * x * 3\"\n");
    expect_inline("a*b*", "text \"a*b*\"\n");
    expect_inline("snake_case_name", "text \"snake_case_name\"\n");
    expect_inline("*spaced *", "text \"*spaced *\"\n| rst.inline.unterminated");
}

#[test]
fn quoted_start_strings_are_text() {
    expect_inline(
        "'*' and \"*\" and (*) and [*] and {*} and <*>",
        "text \"'*' and \\\"*\\\" and (*) and [*] and {*} and <*>\"\n",
    );
    expect_inline("«*» and “*”", "text \"«*» and “*”\"\n");
    // A quote before a start-string without the matching close is fine.
    expect_inline("'*emph*'", "text \"'\"\nemphasis\n  text \"emph\"\ntext \"'\"\n");
    expect_inline("(*emph*)", "text \"(\"\nemphasis\n  text \"emph\"\ntext \")\"\n");
    expect_inline("x *", "text \"x *\"\n");
}

#[test]
fn start_and_end_contexts() {
    expect_inline("-*a*-", "text \"-\"\nemphasis\n  text \"a\"\ntext \"-\"\n");
    expect_inline("/*a*/", "text \"/\"\nemphasis\n  text \"a\"\ntext \"/\"\n");
    expect_inline("x*a*", "text \"x*a*\"\n");
    // The first end-string is followed by "x", so the emphasis runs to the last one.
    expect_inline("*a*x *b*", "emphasis\n  text \"a*x *b\"\n");
    expect_inline("*a*.", "emphasis\n  text \"a\"\ntext \".\"\n");
    expect_inline("—*a*—", "text \"—\"\nemphasis\n  text \"a\"\ntext \"—\"\n");
}

#[test]
fn unterminated_markup_warns_and_stays_text() {
    expect_inline("a *b", "text \"a *b\"\n| rst.inline.unterminated");
    expect_inline("a **b", "text \"a **b\"\n| rst.inline.unterminated");
    expect_inline("a ``b", "text \"a ``b\"\n| rst.inline.unterminated");
    expect_inline("a `b", "text \"a `b\"\n| rst.inline.unterminated");
    expect_inline("a |b", "text \"a |b\"\n| rst.inline.unterminated");
    expect_inline("a _`b", "text \"a _`b\"\n| rst.inline.unterminated");
    // An end-string right after the start-string ends the search.
    expect_inline("****", "text \"****\"\n| rst.inline.unterminated");
}

#[test]
fn backslash_escapes() {
    expect_inline("\\*not emphasis*", "text \"*not emphasis*\"\n");
    expect_inline("a\\\\b", "text \"a\\\\b\"\n");
    expect_inline("H\\ :sub:`2`\\ O", "text \"H\"\nsubscript\n  text \"2\"\ntext \"O\"\n");
    expect_inline("*a\\*b*", "emphasis\n  text \"a*b\"\n");
    expect_inline("\\\\*emph*", "text \"\\\\\"\nemphasis\n  text \"emph\"\n");
    expect_inline("end\\", "text \"end\"\n");
    // An escaped line break joins the lines.
    expect_inline("a\\\nb", "text \"ab\"\n");
}

#[test]
fn inline_literals_keep_backslashes() {
    expect_inline("``a\\*b``", "code \"a\\\\*b\"\n");
    expect_inline("``*x*``", "code \"*x*\"\n");
    expect_inline("``a\\``", "code \"a\\\\\"\n");
    expect_inline("``a``` b", "code \"a`\"\ntext \" b\"\n");
    // Parsing keeps the break; resolution (unbreak_code) turns it into a space.
    expect_inline("``two\nlines``", "code \"two\\nlines\"\n");
}

#[test]
fn simple_references() {
    expect_inline(
        "see name_.",
        "text \"see \"\nreference link=unresolved name=\"name\"\n  text \"name\"\ntext \".\"\n",
    );
    expect_inline("Name_", "reference link=unresolved name=\"name\"\n  text \"Name\"\n");
    expect_inline("a-b.c_", "reference link=unresolved name=\"a-b.c\"\n  text \"a-b.c\"\n");
    expect_inline("a_b_", "reference link=unresolved name=\"a_b\"\n  text \"a_b\"\n");
    expect_inline("anon__", "reference +anonymous link=unresolved\n  text \"anon\"\n");
    expect_inline("a__b", "text \"a__b\"\n");
    expect_inline("a___", "text \"a___\"\n");
    expect_inline("x_.y", "reference link=unresolved name=\"x\"\n  text \"x\"\ntext \".y\"\n");
}

#[test]
fn phrase_references() {
    expect_inline(
        "`A  Phrase`_",
        "reference link=unresolved name=\"a phrase\"\n  text \"A  Phrase\"\n",
    );
    expect_inline(
        "`two\nlines`_",
        concat!(
            "reference link=unresolved name=\"two lines\"\n",
            "  text \"two\"\n  soft-break\n  text \"lines\"\n"
        ),
    );
    expect_inline("`anon`__", "reference +anonymous link=unresolved\n  text \"anon\"\n");
}

#[test]
fn embedded_uris_and_aliases() {
    expect_inline(
        "`Python <https://python.org>`_",
        concat!(
            "link link=external dest=\"https://python.org\" name=\"python\"\n",
            "  text \"Python\"\n"
        ),
    );
    expect_inline(
        "`Python <https://python.org>`__",
        "link link=external dest=\"https://python.org\"\n  text \"Python\"\n",
    );
    expect_inline(
        "`<https://python.org>`_",
        concat!(
            "link link=external dest=\"https://python.org\" name=\"https://python.org\"\n",
            "  text \"https://python.org\"\n"
        ),
    );
    expect_inline(
        "`long <http://a.org/\n   path>`_",
        "link link=external dest=\"http://a.org/path\" name=\"long\"\n  text \"long\"\n",
    );
    expect_inline("`x <a\\ b>`_", "link link=external dest=\"a b\" name=\"x\"\n  text \"x\"\n");
    expect_inline(
        "`Mail <me@example.org>`_",
        "link link=email dest=\"me@example.org\" name=\"mail\"\n  text \"Mail\"\n",
    );
    expect_inline(
        "`Docs <Other Name_>`_",
        concat!(
            "reference link=unresolved name=\"other name\" @target-name=\"docs\"\n",
            "  text \"Docs\"\n"
        ),
    );
    expect_inline(
        "`Docs <other_>`__",
        "reference link=unresolved name=\"other\"\n  text \"Docs\"\n",
    );
    expect_inline(
        "`<other_>`_",
        concat!("reference link=unresolved name=\"other\"\n", "  text \"other\"\n"),
    );
    // An escaped trailing underscore makes a URI, as does a URI-shaped alias.
    expect_inline(
        "`x <file\\_>`_",
        "link link=external dest=\"file_\" name=\"x\"\n  text \"x\"\n",
    );
    expect_inline(
        "`x <http://a.org/b_>`_",
        "link link=external dest=\"http://a.org/b_\" name=\"x\"\n  text \"x\"\n",
    );
    // Without white space before "<" the brackets are part of the text.
    expect_inline("`a<b>`_", "reference link=unresolved name=\"a<b>\"\n  text \"a<b>\"\n");
    expect_inline("`a \\<b>`_", "reference link=unresolved name=\"a <b>\"\n  text \"a <b>\"\n");
}

#[test]
fn inline_targets() {
    expect_inline(
        "a _`Inline Target` b",
        concat!(
            "text \"a \"\ninline-target \"inline target\"\n  text \"Inline Target\"\n",
            "text \" b\"\n"
        ),
    );
}

#[test]
fn footnote_and_citation_references() {
    expect_inline(
        "a [1]_ b",
        "text \"a \"\nfootnote-reference \"1\" link=unresolved name=\"1\"\ntext \" b\"\n",
    );
    expect_inline("[#]_", "footnote-reference +auto \"#\" link=unresolved\n");
    expect_inline(
        "[#Note]_",
        "footnote-reference +auto \"#Note\" link=unresolved name=\"note\"\n",
    );
    expect_inline("[*]_", "footnote-reference +symbol \"*\" link=unresolved\n");
    expect_inline(
        "[CIT2002]_",
        "citation-reference \"CIT2002\" link=unresolved name=\"cit2002\"\n",
    );
    expect_inline("[1a]_", "citation-reference \"1a\" link=unresolved name=\"1a\"\n");
    expect_inline("x[1]_", "text \"x[1]_\"\n");
    expect_inline("[1]_x", "text \"[1]_x\"\n");
    expect_inline("[a b]_", "text \"[a b]_\"\n");
}

#[test]
fn substitution_references() {
    expect_inline(
        "a |Some  Name| b",
        concat!(
            "text \"a \"\nsubstitution-reference \"Some Name\" link=unresolved ",
            "name=\"some name\"\ntext \" b\"\n"
        ),
    );
    expect_inline(
        "|logo|_",
        concat!(
            "reference link=unresolved name=\"logo\"\n",
            "  substitution-reference \"logo\" link=unresolved name=\"logo\"\n"
        ),
    );
    expect_inline(
        "|logo|__",
        concat!(
            "reference +anonymous link=unresolved\n",
            "  substitution-reference \"logo\" link=unresolved name=\"logo\"\n"
        ),
    );
    expect_inline("a || b", "text \"a || b\"\n");
}

#[test]
fn standalone_hyperlinks() {
    expect_inline(
        "see https://example.org/a.",
        concat!(
            "text \"see \"\nlink link=external dest=\"https://example.org/a\"\n",
            "  text \"https://example.org/a\"\ntext \".\"\n"
        ),
    );
    expect_inline(
        "(http://x.org/a)",
        concat!(
            "text \"(\"\nlink link=external dest=\"http://x.org/a\"\n",
            "  text \"http://x.org/a\"\ntext \")\"\n"
        ),
    );
    expect_inline(
        "<http://x.org/a>",
        concat!(
            "text \"<\"\nlink link=external dest=\"http://x.org/a\"\n",
            "  text \"http://x.org/a\"\ntext \">\"\n"
        ),
    );
    expect_inline(
        "http://x.org/q?a=1#frag,",
        concat!(
            "link link=external dest=\"http://x.org/q?a=1#frag\"\n",
            "  text \"http://x.org/q?a=1#frag\"\ntext \",\"\n"
        ),
    );
    expect_inline(
        "mail me@example.org!",
        concat!(
            "text \"mail \"\nlink link=email dest=\"me@example.org\"\n",
            "  text \"me@example.org\"\ntext \"!\"\n"
        ),
    );
    expect_inline(
        "mailto:me@example.org",
        concat!(
            "link link=external dest=\"mailto:me@example.org\"\n",
            "  text \"mailto:me@example.org\"\n"
        ),
    );
    expect_inline("unknown:thing and note: this", "text \"unknown:thing and note: this\"\n");
    // ".." cannot occur in an address, and "." is no start context.
    expect_inline("a..b@c.org", "text \"a..b@c.org\"\n");
    expect_inline(
        "a.b-c@d.org",
        concat!("link link=email dest=\"a.b-c@d.org\"\n", "  text \"a.b-c@d.org\"\n"),
    );
    expect_inline("xhttp://a.org", "text \"xhttp://a.org\"\n");
    expect_inline(
        "a\nftp://b.org/f\nc",
        concat!(
            "text \"a\"\nsoft-break\nlink link=external dest=\"ftp://b.org/f\"\n",
            "  text \"ftp://b.org/f\"\nsoft-break\ntext \"c\"\n"
        ),
    );
}

#[test]
fn hyperlinks_split_by_problematic_markup() {
    // The unterminated "*" separates the URI from the following text.
    expect_inline(
        "http://a.org/*oops",
        concat!(
            "link link=external dest=\"http://a.org/\"\n  text \"http://a.org/\"\n",
            "text \"*oops\"\n| rst.inline.unterminated"
        ),
    );
    expect_inline(
        "x *me@b.org",
        concat!(
            "text \"x *\"\nlink link=email dest=\"me@b.org\"\n  text \"me@b.org\"\n",
            "| rst.inline.unterminated"
        ),
    );
}

#[test]
fn nested_looking_markup() {
    expect_inline("**a *b* c**", "strong\n  text \"a *b* c\"\n");
    // The emphasis ends at the first "*" followed by white space.
    expect_inline("*a **b** c*", "emphasis\n  text \"a **b*\"\ntext \" c*\"\n");
    expect_inline(
        "`a *b*`_",
        concat!("reference link=unresolved name=\"a *b*\"\n", "  text \"a *b*\"\n"),
    );
}

#[test]
fn interpreted_text_problems() {
    expect_inline(":sub:`a`:sup:", "text \":sub:`a`:sup:\"\n| rst.inline.multiple-roles");
    expect_inline(":sub:`a`_", "text \":sub:`a`_\"\n| rst.inline.role-reference");
    expect_inline("`a`:sub:_", "text \"`a`:sub:_\"\n| rst.inline.role-reference");
    expect_inline(":nope:`a`", "text \":nope:`a`\"\n| rst.role.unknown");
}
