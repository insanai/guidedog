//! sphinx.ext.doctest is built in. The tests are never run (there is no Python); the
//! directives show their code as Sphinx does for builders other than doctest:
//! doctest is Code_Block{Language "pycon"}, testcode Code_Block{Language "python"},
//! testoutput Code_Block{Language "none"}, not highlighted. testsetup, testcleanup, and
//! any of them with :hide: become a Comment, which renderers leave out. Every node has
//! Custom "testnodetype" = the directive name and "groups" = the group argument
//! ("default" without one). With trimming (trim_doctest_flags, overridden by
//! :trim-doctest-flags: or :no-trim-doctest-flags:), "# doctest: +FLAG" comments and
//! "<BLANKLINE>" markers are removed from shown sessions: doctest blocks, and testcode
//! blocks that start with ">>>", as Sphinx trims Python console code.

use guidedoc::{AttributeKey, Fail, NodeKind};
use guidedoc_rst::{Directive, OptSpec, OptType, Parser, opt};

use crate::extension::Sphinx;
use crate::text::trim_space;

/// The options of the doctest directives.
pub(crate) const OPTS_DOCTEST: &[OptSpec] = &[
    opt("hide", OptType::Flag),
    opt("options", OptType::Text),
    opt("pyversion", OptType::Text),
    opt("skipif", OptType::Text),
    opt("trim-doctest-flags", OptType::Flag),
    opt("no-trim-doctest-flags", OptType::Flag),
];

/// Runs doctest, testcode, testoutput, testsetup, or testcleanup (`name`).
pub(crate) fn doctest_directive(
    sx: &Sphinx<'_>,
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
    name: &str,
) -> Result<(), Fail> {
    let hidden = name == "testsetup" || name == "testcleanup" || d.has_option("hide");
    let id = p.open(if hidden { NodeKind::Comment } else { NodeKind::CodeBlock }, d.span)?;
    if !hidden {
        let language = match name {
            "testcode" => "python",
            "testoutput" => "none",
            _ => "pycon",
        };
        p.ctx.b.add_attribute_string(id, AttributeKey::Language, language)?;
    }
    p.custom(id, "testnodetype", name)?;
    let arg = d.argument();
    let groups = trim_space(&arg);
    p.custom(id, "groups", if groups.is_empty() { "default" } else { groups })?;
    let mut trim = sx.config.trim_doctest_flags;
    if d.has_option("trim-doctest-flags") {
        trim = true;
    }
    if d.has_option("no-trim-doctest-flags") {
        trim = false;
    }
    let console = d.content.first().is_some_and(|l| l.text.starts_with(">>>"));
    let trim = trim && !hidden && (name == "doctest" || (name == "testcode" && console));
    let mut text = String::new();
    for l in &d.content {
        text.push_str(if trim { trim_doctest_line(l.text) } else { l.text });
        text.push('\n');
    }
    let t = p.ctx.b.add_text(&text)?;
    p.ctx.b.set_text(id, t);
    p.close(id)
}

/// Removes a "<BLANKLINE>" marker (with the white space before it) from the start of a
/// line, and a "# doctest: ..." comment (with the blanks before it) from its end, as
/// Sphinx does for presentation.
pub(crate) fn trim_doctest_line(t: &str) -> &str {
    let blanks = |c: char| c == ' ' || c == '\t';
    let mut line = t;
    if let Some(rest) = t.trim_start_matches(blanks).strip_prefix("<BLANKLINE>") {
        line = rest;
    }
    let b = line.as_bytes();
    for i in 0..b.len() {
        if b[i] != b'#' {
            continue;
        }
        let after = line[i + 1..].trim_start_matches(blanks);
        if after.len() <= 8 || !after.starts_with("doctest:") {
            continue;
        }
        return line[..i].trim_end_matches(blanks);
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctest_flags_and_blank_lines_are_trimmed() {
        assert_eq!(trim_doctest_line(">>> f()  # doctest: +SKIP"), ">>> f()");
        assert_eq!(trim_doctest_line("  <BLANKLINE>"), "");
        assert_eq!(trim_doctest_line("x # other"), "x # other");
    }
}
