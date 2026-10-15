use guidedoc::{
    AttributeKey as A, Builder, ContentPolicy, Fail, LinkKind, Message, NodeFlags, NodeKind as K,
    Severity, SourceSpan, Status, default_policy, trusted_policy,
};

use super::*;
use crate::escape::url_unsafe;

#[test]
fn text_and_attributes_are_escaped() {
    let r = render(
        |b| {
            open(b, K::Paragraph)?;
            text(b, "a<b & \"c\" > d")?;
            link_titled(b, K::Link, "/a b?x=1&y=\"2\"%41%zzé", LinkKind::External, "t\"<")?;
            text(b, "go")?;
            shut(b)?;
            shut(b)
        },
        frag(),
    );
    assert_eq!(r.status, Status::Ok);
    assert_eq!(
        r.html,
        "<p>a&lt;b &amp; &quot;c&quot; &gt; d<a href=\
         \"/a%20b?x=1&amp;y=%222%22%41%25zz%C3%A9\" title=\"t&quot;&lt;\">go</a></p>\n"
    );
}

// Docutils' writer encodes @ as &#64; in text and attributes (ids and links included);
// code, as Pygments writes it, and permalinks, which Sphinx writes itself, keep it.
#[test]
fn at_signs_are_encoded_as_docutils_does() {
    let r = render(
        |b| {
            open_v(b, K::Section, 1)?;
            attr(b, A::Id, "c.A.@data")?;
            open_v(b, K::Heading, 1)?;
            text(b, "a@b")?;
            shut(b)?;
            leaf(b, K::CodeBlock, "@decorator\n")?;
            open(b, K::Paragraph)?;
            link(b, K::Link, "#c.A.@data", LinkKind::External)?;
            text(b, "x")?;
            shut(b)?;
            shut(b)?;
            shut(b)
        },
        RenderOptions { permalinks: true, ..frag() },
    );
    has(&r, "<section id=\"c.A.&#64;data\">");
    has(&r, "a&#64;b<a class=\"headerlink\" href=\"#c.A.@data\"");
    has(&r, "<code>@decorator\n</code>");
    has(&r, "href=\"#c.A.&#64;data\"");
}

fn unsafe_link(b: &mut Builder<'_>) -> Result<(), Fail> {
    open(b, K::Paragraph)?;
    link(b, K::Link, " JaVa\tScript:alert(1)", LinkKind::External)?;
    text(b, "x")?;
    shut(b)?;
    shut(b)
}

#[test]
fn unsafe_urls_follow_the_policy() {
    let refused = render_with(unsafe_link, frag(), default_policy(), |_| {});
    assert_eq!(refused.status, Status::Policy);
    assert_eq!(refused.code, "html.url.unsafe");
    let omit = Policy { unsafe_urls: ContentPolicy::Omit, ..default_policy() };
    let omitted = render_with(unsafe_link, frag(), omit, |_| {});
    assert_eq!(omitted.status, Status::Ok);
    assert_eq!(omitted.code, "html.url.omitted");
    assert_eq!(omitted.html, "<p><a>x</a></p>\n");
    let allowed = render(unsafe_link, frag());
    has(&allowed, "href=\"%20JaVa%09Script:alert(1)\"");
    for dest in ["vbscript:x", "FILE:///etc/passwd", "data:text/html,<b>", "javascript:x"] {
        assert!(url_unsafe(dest), "{dest}");
    }
    for dest in [
        "data:image/png;base64,AA",
        "DATA:image/WEBP,x",
        "https://x.org",
        "/relative:colon",
        "mailto:a@b",
        "#frag",
    ] {
        assert!(!url_unsafe(dest), "{dest}");
    }
    let image = render_with(
        |b| {
            open(b, K::Paragraph)?;
            link(b, K::Image, "data:image/gif;base64,R0", LinkKind::External)?;
            text(b, "alt *text*")?;
            shut(b)?;
            link(b, K::Link, "me@example.org", LinkKind::Email)?;
            text(b, "mail")?;
            shut(b)?;
            shut(b)
        },
        frag(),
        default_policy(),
        |_| {},
    );
    assert_eq!(image.status, Status::Ok);
    assert_eq!(
        image.html,
        "<p><img src=\"data:image/gif;base64,R0\" alt=\"alt *text*\" />\
         <a href=\"mailto:me&#64;example.org\">mail</a></p>\n"
    );
}

fn raw_with_format(b: &mut Builder<'_>) -> Result<(), Fail> {
    let id = leaf(b, K::RawBlock, "<div>x</div>\n")?;
    b.add_attribute_string(id, A::Format, "html")?;
    let id = leaf(b, K::RawBlock, "#set page(width: 1cm)\n")?;
    b.add_attribute_string(id, A::Format, "typst")?;
    open(b, K::Paragraph)?;
    let id = leaf(b, K::RawInline, "<b>")?;
    b.add_attribute_string(id, A::Format, "HTML")?;
    shut(b)
}

#[test]
fn raw_content_follows_the_policy() {
    let refused = render_with(raw_with_format, frag(), default_policy(), |_| {});
    assert_eq!(refused.status, Status::Policy);
    assert_eq!(refused.code, "html.raw.refused");
    let omit = Policy { raw: ContentPolicy::Omit, ..default_policy() };
    let omitted = render_with(raw_with_format, frag(), omit, |_| {});
    assert_eq!(omitted.status, Status::Ok);
    assert_eq!(omitted.warnings, 3);
    assert_eq!(omitted.code, "html.raw.omitted");
    assert_eq!(omitted.html, "<p></p>\n");
    let allowed = render(raw_with_format, frag());
    assert_eq!(allowed.status, Status::Ok);
    assert_eq!(allowed.code, "html.raw.format");
    assert_eq!(allowed.html, "<div>x</div>\n<p><b></p>\n");
    let untyped = render(
        |b| {
            leaf(b, K::RawBlock, "<div>x</div>\n")?;
            para(b, "after")
        },
        frag(),
    );
    assert_eq!(untyped.code, "html.raw.format");
    lacks(&untyped, "<div>");
}

fn inline_kinds(b: &mut Builder<'_>) -> Result<(), Fail> {
    let def = open(b, K::SubstitutionDefinition)?;
    set_text(b, def, "name")?;
    inline(b, K::Strong, "Guidedoc")?;
    shut(b)?;
    open(b, K::Paragraph)?;
    inline(b, K::TitleReference, "Dune")?;
    open(b, K::Abbreviation)?;
    attr(b, A::Title, "HyperText \"Markup\"")?;
    text(b, "HTML")?;
    shut(b)?;
    let role = open(b, K::Role)?;
    set_text(b, role, "kbd")?;
    attr(b, A::Class, "key")?;
    text(b, "Ctrl")?;
    shut(b)?;
    inline(b, K::Subscript, "2")?;
    inline(b, K::Superscript, "n")?;
    leaf(b, K::Math, "a<b")?;
    let target = open(b, K::InlineTarget)?;
    set_text(b, target, "spot")?;
    text(b, "here")?;
    shut(b)?;
    let row = link(b, K::Reference, "", LinkKind::External)?;
    text(b, "jump")?;
    shut(b)?;
    b.resolve_link(row, target, LinkKind::Internal, guidedoc::Text::default());
    let sub = open(b, K::SubstitutionReference)?;
    set_text(b, sub, "name")?;
    set_link(b, sub, LinkKind::Internal, def)?;
    shut(b)?;
    let code = leaf(b, K::Code, "x := 1")?;
    b.add_attribute_string(code, A::Language, "odin")?;
    leaf(b, K::SoftBreak, "")?;
    leaf(b, K::HardBreak, "")?;
    link(b, K::Link, "book-key", LinkKind::Project)?;
    text(b, "chapter")?;
    shut(b)?;
    shut(b)
}

#[test]
fn inline_kinds_render_semantically() {
    let r = render(inline_kinds, frag());
    assert_eq!(r.status, Status::Ok);
    assert_eq!(
        r.html,
        "<p><cite>Dune</cite><abbr title=\"HyperText &quot;Markup&quot;\">HTML</abbr>\
         <kbd class=\"kbd docutils literal notranslate key\">Ctrl</kbd><sub>2</sub>\
         <sup>n</sup><span class=\"math\">\\(a&lt;b\\)</span><span id=\"spot\">here</span>\
         <a href=\"#spot\">jump</a><strong>Guidedoc</strong><code data-language=\"odin\">\
         x := 1</code>\n<br />\n<a href=\"book-key\">chapter</a></p>\n"
    );
    let unresolved = render(
        |b| {
            open(b, K::Paragraph)?;
            link(b, K::Reference, "", LinkKind::Unresolved)?;
            text(b, "missing_")?;
            shut(b)?;
            shut(b)
        },
        frag(),
    );
    assert_eq!(unresolved.status, Status::InvalidInput);
    assert_eq!(unresolved.code, "html.reference.unresolved");
}

/// Stands for a reader's content error, such as an unknown directive.
const UNKNOWN_CONSTRUCT: Message = Message {
    code: "test.unknown",
    severity: Severity::Error,
    status: Status::Unsupported,
    title: "UNKNOWN",
    message: "Unknown.",
    hint: "Remove it.",
};

fn kept_tree(b: &mut Builder<'_>) -> Result<(), Fail> {
    let none = guidedoc::Arg::None;
    b.report(UNKNOWN_CONSTRUCT, SourceSpan::default(), none.clone(), none);
    para(b, "kept")?;
    open(b, K::Paragraph)?;
    link(b, K::Reference, "", LinkKind::Unresolved)?;
    text(b, "missing_")?;
    shut(b)?;
    let sub = leaf(b, K::SubstitutionReference, "name")?;
    set_link(b, sub, LinkKind::Unresolved, guidedoc::INVALID_NODE)?;
    shut(b)
}

#[test]
fn kept_documents_render_problems_as_problematic_text() {
    let refused = render(kept_tree, frag());
    assert_eq!(refused.status, Status::Unsupported);
    let keep = Policy { keep_documents_with_errors: true, ..trusted_policy() };
    let r = render_with(kept_tree, frag(), keep, |_| {});
    assert_eq!(r.status, Status::Ok);
    assert_eq!(r.code, "test.unknown");
    assert_eq!(r.warnings, 2);
    assert_eq!(
        r.html,
        "<p>kept</p>\n<p><span id=\"id4\" class=\"problematic\">missing_</span>\
         <span id=\"id2\" class=\"problematic\">|name|</span></p>\n"
    );
}

fn media(b: &mut Builder<'_>) -> Result<(), Fail> {
    link(b, K::Image, "clips/intro.MP4", LinkKind::External)?;
    attr(b, A::Alt, "The intro")?;
    attr(b, A::Width, "640")?;
    shut(b)?;
    link(b, K::Image, "theme.ogg?v=2", LinkKind::External)?;
    shut(b)?;
    link(b, K::Image, "photo.png", LinkKind::External)?;
    shut(b)
}

// Video and audio files named by an image play, with a link for browsers that cannot.
#[test]
fn video_and_audio_files_play() {
    let r = render(media, frag());
    assert_eq!(r.status, Status::Ok);
    has(
        &r,
        "<video src=\"clips/intro.MP4\" controls title=\"The intro\" width=\"640\">\
         <a href=\"clips/intro.MP4\">The intro</a></video>",
    );
    has(
        &r,
        "<audio src=\"theme.ogg?v=2\" controls>\
         <a href=\"theme.ogg?v=2\">theme.ogg?v=2</a></audio>",
    );
    has(&r, "<img src=\"photo.png\" alt=\"\" />");
}

const DEEP_QUOTES: usize = 200_000;

// The walk keeps open nodes in an explicit stack, so a tree nested far deeper than any
// thread's stack could hold renders when max_depth allows it.
#[test]
fn deep_trees_render_without_recursion() {
    let r = render_with(
        |b| {
            for _ in 0..DEEP_QUOTES {
                open(b, K::BlockQuote)?;
            }
            para(b, "deep")?;
            for _ in 0..DEEP_QUOTES {
                shut(b)?;
            }
            Ok(())
        },
        frag(),
        trusted_policy(),
        |request| request.limits.max_depth = 1_000_000,
    );
    assert_eq!(r.status, Status::Ok);
    assert_eq!(r.html.matches("<blockquote>").count(), DEEP_QUOTES);
    assert!(r.html.contains("<p>deep</p>"));
}

#[test]
fn symbols_flags_are_kept() {
    // A symbol footnote reference past the tenth symbol repeats it.
    let r = render(
        |b| {
            open(b, K::Paragraph)?;
            let star = open_v(b, K::FootnoteReference, 12)?;
            set_text(b, star, "*")?;
            b.add_flags(star, NodeFlags::SYMBOL);
            set_link(b, star, LinkKind::External, guidedoc::INVALID_NODE)?;
            shut(b)?;
            shut(b)
        },
        frag(),
    );
    has(&r, ">[††]</a>");
}
