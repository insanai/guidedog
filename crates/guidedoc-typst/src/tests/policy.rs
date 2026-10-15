//! Policies (raw content, unsafe URLs, untranslatable math), diagnostics, and options.

use std::cell::Cell;

use guidedoc::{
    Arg, AttributeKey as Key, ContentPolicy, INVALID_NODE, LinkKind, Message, NodeId,
    NodeKind as K, Policy, RenderOptions, Severity, SourceSpan, Status, trusted_policy,
};

use super::{Tb, expect_compiles, refusing, render_policy, render_with};
use crate::tables::parse_length;

type R = Result<(), guidedoc::Fail>;

thread_local! {
    /// The text and second parameter (format, link kind) of the current test's leaf.
    static LEAF: Cell<(&'static str, &'static str)> = const { Cell::new(("", "")) };
    static LINK: Cell<LinkKind> = const { Cell::new(LinkKind::External) };
}

fn with_raw(policy: ContentPolicy) -> Policy {
    Policy { raw: policy, ..Policy::default() }
}

fn build_raw(t: &mut Tb<'_, '_>) -> R {
    let (text, format) = LEAF.get();
    let raw = t.leaf(K::RawBlock, text, 0)?;
    t.attr(raw, Key::Format, format)?;
    t.para("kept")
}

fn render_raw(text: &'static str, format: &'static str, policy: ContentPolicy) -> super::Rendered {
    LEAF.set((text, format));
    render_policy(build_raw, with_raw(policy))
}

#[test]
fn raw_typst_follows_policy() {
    let r = render_raw("#panic(\"raw\")\n", "typst", ContentPolicy::Refuse);
    assert_eq!(r.status, Status::Policy);
    assert!(r.has_code("typst.raw.refused", Severity::Error));
    let r = render_raw("#emph[raw]\n", "typst", ContentPolicy::Omit);
    assert_eq!(r.status, Status::Ok);
    assert!(r.has_code("typst.raw.omitted", Severity::Warning));
    assert!(!r.output.contains("#emph[raw]"));
    r.expect("kept");
    let r = render_raw("#emph[raw]", "Typst", ContentPolicy::Allow);
    assert_eq!(r.status, Status::Ok);
    r.expect("\n#emph[raw]\n");
}

#[test]
fn raw_foreign_formats_are_never_emitted() {
    for policy in [ContentPolicy::Refuse, ContentPolicy::Omit, ContentPolicy::Allow] {
        let r = render_raw("<b>html</b>", "html", policy);
        assert!(!r.output.contains("<b>"));
        if policy == ContentPolicy::Refuse {
            assert_eq!(r.status, Status::Unsupported);
            assert!(r.has_code("typst.raw.foreign", Severity::Error));
        } else {
            assert_eq!(r.status, Status::Ok);
            assert!(r.has_code("typst.raw.foreign", Severity::Warning));
        }
    }
}

fn build_math(t: &mut Tb<'_, '_>) -> R {
    let (text, _) = LEAF.get();
    let p = t.open(K::Paragraph, 0)?;
    t.text("Before ")?;
    t.leaf(K::Math, text, 0)?;
    t.close(p)?;
    t.leaf(K::MathBlock, text, 0).map(|_| ())
}

fn render_math(text: &'static str, policy: ContentPolicy) -> super::Rendered {
    LEAF.set((text, ""));
    render_policy(build_math, Policy { math: policy, ..Policy::default() })
}

#[test]
fn untranslatable_math_follows_policy() {
    let r = render_math("\\foo{x} + 1", ContentPolicy::Refuse);
    assert_eq!(r.status, Status::Unsupported);
    assert!(r.has_code("typst.math.unsupported", Severity::Error));
    let primary = r.reports.iter().find(|d| d.severity == Severity::Error);
    assert_eq!(primary.map(|d| d.args[0].clone()), Some(Arg::Text("\\foo".into())));
    let r = render_math("\\foo{x} + 1", ContentPolicy::Omit);
    assert_eq!(r.status, Status::Ok);
    assert!(r.has_code("typst.math.unsupported", Severity::Warning));
    assert!(!r.output.contains("\\foo") && !r.output.contains('$'));
    let r = render_math("\\foo{x} + 1", ContentPolicy::Allow);
    assert_eq!(r.status, Status::Ok);
    assert!(r.has_code("typst.math.unsupported", Severity::Warning));
    r.expect("#raw(lang: \"latex\", \"\\\\foo{x} + 1\")");
    r.expect("#raw(block: true, lang: \"latex\", \"\\\\foo{x} + 1\")");
    let r = render_math("\\frac{1}{2}", ContentPolicy::Refuse);
    assert_eq!(r.status, Status::Ok);
    r.expect("Before #context { let gd-math-font = ");
    r.expect("; $frac(1, 2)$ }");
    r.expect("$ frac(1, 2) $ }\n");
}

fn build_link(t: &mut Tb<'_, '_>) -> R {
    let kind = LINK.get();
    let (destination, _) = LEAF.get();
    let p = t.open(K::Paragraph, 0)?;
    let link_kind = if kind == LinkKind::Unresolved { K::Reference } else { K::Link };
    let link = t.open(link_kind, 0)?;
    t.link(link, kind, destination, "missing target", INVALID_NODE)?;
    t.text("link text")?;
    t.close(link)?;
    t.close(p)
}

fn render_link(kind: LinkKind, destination: &'static str, urls: ContentPolicy) -> super::Rendered {
    LINK.set(kind);
    LEAF.set((destination, ""));
    render_policy(build_link, Policy { unsafe_urls: urls, ..Policy::default() })
}

#[test]
fn links_follow_resolution_and_url_policy() {
    let r = render_link(LinkKind::Unresolved, "", ContentPolicy::Refuse);
    assert_eq!(r.status, Status::InvalidInput);
    assert!(r.has_code("typst.link.unresolved", Severity::Error));
    let r = render_link(LinkKind::External, " JavaScript:alert(1)", ContentPolicy::Refuse);
    assert_eq!(r.status, Status::Policy);
    assert!(r.has_code("typst.url.unsafe", Severity::Error));
    let r = render_link(LinkKind::External, " JavaScript:alert(1)", ContentPolicy::Omit);
    assert_eq!(r.status, Status::Ok);
    assert!(r.has_code("typst.url.unsafe", Severity::Warning));
    assert!(!r.output.contains("alert"));
    r.expect("link text");
    let r = render_link(LinkKind::External, "data:text/html,x", ContentPolicy::Refuse);
    assert_eq!(r.status, Status::Policy);
    let r = render_link(LinkKind::Project, "chapter-2-intro", ContentPolicy::Refuse);
    assert_eq!(r.status, Status::Ok);
    r.expect("#gd-link(\"chapter-2-intro\")[link text]");
}

fn build_images(t: &mut Tb<'_, '_>) -> R {
    let image = t.open(K::Image, 0)?;
    t.attr(image, Key::Alt, "remote")?;
    t.attr(image, Key::Width, "wide")?;
    t.link(image, LinkKind::External, "https://example.com/i.png", "", INVALID_NODE)?;
    t.close(image)?;
    let local = t.open(K::Image, 0)?;
    t.attr(local, Key::Width, "2.5in")?;
    t.attr(local, Key::Height, "3em")?;
    t.link(local, LinkKind::External, "images/a b.png", "", INVALID_NODE)?;
    t.close(local)
}

#[test]
fn images_convert_sizes_and_link_remote_files() {
    let r = render_policy(build_images, refusing());
    assert_eq!(r.status, Status::Ok);
    assert!(r.has_code("typst.image.remote", Severity::Warning));
    r.expect("#link(\"https://example.com/i.png\", [remote])");
    r.expect("#image(\"images/a b.png\", width: 2.5in, height: 3em)");
    assert_eq!(parse_length("12.25 px"), Some((12250, "px")));
    assert_eq!(parse_length("12furlongs"), None);
}

fn build_notes(t: &mut Tb<'_, '_>) -> R {
    let p = t.open(K::Paragraph, 0)?;
    t.leaf(K::FootnoteReference, "3", 0)?;
    let star = t.leaf(K::FootnoteReference, "*", 1)?;
    t.b.add_flags(star, guidedoc::NodeFlags::SYMBOL);
    t.leaf(K::FootnoteReference, "9", 0)?;
    t.leaf(K::SubstitutionReference, "loop", 0)?;
    t.leaf(K::CitationReference, "nowhere", 0)?;
    t.close(p)?;
    let note = t.open(K::Footnote, 0)?;
    t.set_text(note, "3")?;
    t.para("Third.")?;
    t.close(note)?;
    let symbol = t.open(K::Footnote, 1)?;
    t.set_text(symbol, "*")?;
    t.b.add_flags(symbol, guidedoc::NodeFlags::SYMBOL | guidedoc::NodeFlags::AUTO);
    t.para("Dagger.")?;
    t.close(symbol)?;
    let definition = t.open(K::SubstitutionDefinition, 0)?;
    t.set_text(definition, "loop")?;
    t.leaf(K::SubstitutionReference, "loop", 0)?;
    t.close(definition)
}

#[test]
fn footnotes_match_by_label_and_symbol() {
    let r = render_policy(build_notes, refusing());
    assert_eq!(r.status, Status::Ok, "{:?}", r.reports);
    r.expect("#gd-footnote(\"3\")[\nThird\\.\n]<id5>");
    r.expect("#gd-footnote(\"*\")[\nDagger\\.\n]");
    assert!(r.has_code("typst.footnote.missing", Severity::Warning));
    assert!(r.has_code("typst.substitution.missing", Severity::Warning));
    assert!(r.has_code("typst.citation.missing", Severity::Warning));
    r.expect("\\[nowhere\\]");
}

fn build_plain(t: &mut Tb<'_, '_>) -> R {
    let s = t.section(1, "Only heading")?;
    t.para("Body.")?;
    t.close(s)
}

#[test]
fn options_set_title_language_numbering_and_prefix() {
    let options = RenderOptions {
        language: "pt-BR",
        section_numbers: true,
        anchor_prefix: "Book",
        ..RenderOptions::default()
    };
    let r = render_with(build_plain, Policy::default(), options);
    assert_eq!(r.status, Status::Ok);
    r.expect("#set document(title: \"Only heading\")");
    r.expect("lang: \"pt\", region: \"BR\"");
    r.expect("gd-sectnum(99, \"\", \"\", 1)");
    r.expect("<book-only-heading>");
    let options =
        RenderOptions { title: "Override \"q\"", language: "x", ..RenderOptions::default() };
    let r = render_with(build_plain, Policy::default(), options);
    assert!(r.has_code("typst.language.invalid", Severity::Warning));
    r.expect("title: \"Override \\\"q\\\"\"");
    r.expect("lang: \"en\"");
}

fn build_heading_targets(t: &mut Tb<'_, '_>) -> R {
    let contents = t.open(K::Contents, 0)?;
    t.close(contents)?;
    let section = t.open(K::Section, 1)?;
    let h = t.open(K::Heading, 1)?;
    t.text("Heading ")?;
    let target = t.open(K::InlineTarget, 0)?;
    t.set_text(target, "inside")?;
    t.text("target")?;
    t.close(target)?;
    let reference = t.leaf(K::FootnoteReference, "1", 1)?;
    t.link(reference, LinkKind::Internal, "", "", NodeId(0))?;
    t.close(h)?;
    let p = t.open(K::Paragraph, 0)?;
    t.leaf(K::FootnoteReference, "1", 1)?;
    let link = t.open(K::Reference, 0)?;
    t.link(link, LinkKind::Internal, "", "", target)?;
    t.text("to the target")?;
    t.close(link)?;
    t.close(p)?;
    let note = t.open(K::Footnote, 1)?;
    t.set_text(note, "1")?;
    t.para("In a heading.")?;
    t.close(note)?;
    t.close(section)
}

/// Typst repeats heading content in the outline, so labels inside headings would occur
/// twice; links to targets inside a heading go to the heading instead.
#[test]
fn headings_carry_no_inner_labels() {
    let r = render_policy(build_heading_targets, refusing());
    assert_eq!(r.status, Status::Ok, "{:?}", r.reports);
    // The title's name holds the number.
    r.expect(
        "#heading(level: 1)[Heading target#gd-footnote(\"1\")[\nIn a heading\\.\n]]\
         <heading-target1>",
    );
    r.expect("#super(\"1\")#gd-link(\"heading-target1\")[to the target]");
    expect_compiles("heading-targets", &r.output, false);
}

fn build_title_directive(t: &mut Tb<'_, '_>) -> R {
    let d = t.open(K::Directive, 0)?;
    t.attr(d, Key::Name, "title")?;
    t.attr(d, Key::Title, "Directive \\ title")?;
    t.close(d)?;
    build_plain(t)
}

#[test]
fn title_directive_names_the_document() {
    let r = render_policy(build_title_directive, refusing());
    assert_eq!(r.status, Status::Ok);
    r.expect("#set document(title: \"Directive \\\\ title\")");
    r.expect("\n#heading(level: 1)[Only heading]<only-heading>\n");
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

fn build_kept(t: &mut Tb<'_, '_>) -> R {
    t.b.report(UNKNOWN_CONSTRUCT, SourceSpan::default(), Arg::None, Arg::None);
    t.para("kept")?;
    let p = t.open(K::Paragraph, 0)?;
    let x = t.open(K::Reference, 0)?;
    t.link(x, LinkKind::Unresolved, "", "missing", INVALID_NODE)?;
    t.text("missing")?;
    t.close(x)?;
    t.close(p)
}

#[test]
fn kept_documents_render_unresolved_references_as_text() {
    let body = RenderOptions { standalone: false, ..RenderOptions::default() };
    let refused = render_with(build_kept, trusted_policy(), body);
    assert_eq!(refused.status, Status::Unsupported);
    let keep = Policy { keep_documents_with_errors: true, ..trusted_policy() };
    let r = render_with(build_kept, keep, body);
    assert_eq!(r.status, Status::Ok, "{:?}", r.reports);
    assert!(r.has_code("test.unknown", Severity::Error));
    assert!(r.has_code("typst.link.unresolved", Severity::Warning));
    assert_eq!(r.output, "kept\n\nmissing\n");
}
