//! Ported from the Odin package's roles_test.odin.

#[allow(unused_imports)]
use crate::config::{Config, TocParents};
#[allow(unused_imports)]
use crate::tests::*;

#[test]
fn standard_domain_roles() {
    expect_dump(
        concat!(
            ":doc:`../intro` :doc:`Intro <intro>` :ref:`My-Label` :numref:`fig <Fig-1>` ",
            ":term:`API` :keyword:`if` :token:`expr` :download:`a.zip` :any:`x`\n",
        ),
        concat!(
            "document value=1\n  paragraph\n",
            "    xref link=unresolved name=\"../intro\" @reftype=\"doc\"\n      ",
            "text \"../intro\"\n",
            "    text \" \"\n",
            "    xref link=unresolved name=\"intro\" @reftype=\"doc\" @refexplicit=\"1\"\n",
            "      text \"Intro\"\n    text \" \"\n",
            "    xref link=unresolved name=\"my-label\" @reftype=\"ref\"\n",
            "      text \"My-Label\"\n    text \" \"\n",
            "    xref link=unresolved name=\"fig-1\" @reftype=\"numref\" @refexplicit=\"1\"\n",
            "      code \"fig\"\n    text \" \"\n",
            "    xref link=unresolved name=\"API\" @reftype=\"term\"\n      text \"API\"\n",
            "    text \" \"\n",
            "    xref link=unresolved name=\"if\" @reftype=\"keyword\"\n      code \"if\"\n",
            "    text \" \"\n",
            "    xref link=unresolved name=\"expr\" @reftype=\"token\"\n      code \"expr\"\n",
            "    text \" \"\n",
            "    xref link=unresolved name=\"a.zip\" @reftype=\"download\"\n",
            "      code \"a.zip\"\n    text \" \"\n",
            "    xref link=unresolved name=\"x\" @reftype=\"any\" @cpp:name=\"x\x1f\"\n",
            "      code \"x\"\n",
        ),
        None,
    );
}

#[test]
fn option_and_envvar_roles() {
    expect_dump(
        ".. program:: rm\n\n:option:`-r` :envvar:`PATH`\n",
        concat!(
            "document value=1\n  paragraph\n",
            "    xref link=unresolved name=\"-r\" @reftype=\"option\" @std:program=\"rm\"\n",
            "      code \"-r\"\n    text \" \"\n",
            "    index @id=\"index-0\" @entries=\"single: PATH\\nsingle: environment variable; ",
            "PATH\"\n",
            "    xref link=unresolved name=\"PATH\" @reftype=\"envvar\"\n      code \"PATH\"\n",
        ),
        None,
    );
}

#[test]
fn python_roles_process_links() {
    expect_dump(
        concat!(
            ".. py:module:: pkg\n\n",
            ":py:func:`~pkg.mod.run` :meth:`.Cls.go()` :class:`!Plain` :func:`Title <f>` ",
            ":py:attr:`a.b`\n",
        ),
        concat!(
            "document value=1\n",
            "  index @id=\"module-pkg\" @entries=\"pair: module; pkg\" @module=\"pkg\"",
            " @domain=\"py\"\n",
            "  paragraph\n",
            "    xref link=unresolved name=\"pkg.mod.run\" @reftype=\"py:func\" ",
            "@py:module=\"pkg\"\n      code \"run()\"\n    text \" \"\n",
            "    xref link=unresolved name=\"Cls.go\" @reftype=\"py:meth\" @refspecific=\"1\" ",
            "@py:module=\"pkg\"\n      code \"Cls.go()\"\n    text \" \"\n",
            "    code \"Plain\"\n    text \" \"\n",
            "    xref link=unresolved name=\"f\" @reftype=\"py:func\" @refexplicit=\"1\" ",
            "@py:module=\"pkg\"\n      code \"Title\"\n    text \" \"\n",
            "    xref link=unresolved name=\"a.b\" @reftype=\"py:attr\" @py:module=\"pkg\"\n",
            "      code \"a.b\"\n",
        ),
        None,
    );
}

#[test]
fn function_parentheses_follow_config() {
    let mut config = Config::default();
    config.add_function_parentheses = false;
    expect_dump(
        ":func:`f()`\n",
        concat!(
            "document value=1\n  paragraph\n",
            "    xref link=unresolved name=\"f\" @reftype=\"py:func\"\n",
            "      code \"f\"\n",
        ),
        Some(&config),
    );
}

#[test]
fn other_domain_roles() {
    expect_contains(
        concat!(
            ":c:func:`~S.run` :cpp:class:`~ns::Klass` :js:func:`$.get` :rst:dir:`toctree` ",
            ":rst:role:`ref` :c:expr:`a + b` :cpp:texpr:`int` :js:class:`Widget`\n",
        ),
        &[
            "name=\"S.run\" @reftype=\"c:func\"",
            "code \"run()\"",
            "name=\"ns::Klass\" @reftype=\"cpp:class\"",
            "code \"Klass\"",
            "name=\"$.get\" @reftype=\"js:func\"",
            "code \"$.get()\"",
            "name=\"toctree\" @reftype=\"rst:dir\"",
            "name=\"ref\" @reftype=\"rst:role\"",
            "role \"c-expr\"\n      xref link=unresolved name=\"a\" @reftype=\"c:identifier\"",
            "text \" + \"\n      xref link=unresolved name=\"b\"",
            "role \"cpp-texpr\"\n      text \"int\"",
            "name=\"Widget\" @reftype=\"js:class\"",
            "code \"Widget\"\n",
        ],
        None,
    );
}

#[test]
fn default_domain_changes_role_lookup() {
    expect_contains(
        ".. default-domain:: c\n\n:func:`f` :ref:`x`\n",
        &["@reftype=\"c:func\"", "@reftype=\"ref\""],
        None,
    );
}

#[test]
fn text_roles() {
    expect_dump(
        concat!(
            ":command:`rm` :dfn:`term` :program:`git` :regexp:`a+` :mimetype:`text/html` ",
            ":newsgroup:`comp.lang` :mailheader:`To` :makevar:`CC`\n",
        ),
        concat!(
            "document value=1\n  paragraph\n",
            "    role \"command\"\n      text \"rm\"\n    text \" \"\n",
            "    role \"dfn\"\n      text \"term\"\n    text \" \"\n",
            "    role \"program\"\n      text \"git\"\n    text \" \"\n",
            "    role \"regexp\"\n      text \"a+\"\n    text \" \"\n",
            "    role \"mimetype\"\n      text \"text/html\"\n    text \" \"\n",
            "    role \"newsgroup\"\n      text \"comp.lang\"\n    text \" \"\n",
            "    role \"mailheader\"\n      text \"To\"\n    text \" \"\n",
            "    role \"makevar\"\n      text \"CC\"\n",
        ),
        None,
    );
}

#[test]
fn kbd_role_splits_keys() {
    expect_dump(
        ":kbd:`Ctrl+X` :kbd:`Caps Lock` :kbd:`q`\n",
        concat!(
            "document value=1\n  paragraph\n",
            "    role \"kbd\" @class=\"compound\"\n",
            "      role \"kbd\"\n        text \"Ctrl\"\n      text \"+\"\n",
            "      role \"kbd\"\n        text \"X\"\n    text \" \"\n",
            "    role \"kbd\"\n      text \"Caps Lock\"\n    text \" \"\n",
            "    role \"kbd\"\n      text \"q\"\n",
        ),
        None,
    );
}

#[test]
fn gui_roles_mark_accelerators() {
    expect_dump(
        ":guilabel:`&Cancel` :menuselection:`&File --> Save && &Quit`\n",
        concat!(
            "document value=1\n  paragraph\n",
            "    role \"guilabel\"\n      role \"accelerator\"\n        text \"C\"\n",
            "      text \"ancel\"\n    text \" \"\n",
            "    role \"menuselection\"\n      role \"accelerator\"\n        text \"F\"\n",
            "      text \"ile ‣ Save & \"\n      role \"accelerator\"\n        text \"Q\"\n",
            "      text \"uit\"\n",
        ),
        None,
    );
}

#[test]
fn samp_file_abbr_manpage() {
    expect_dump(
        concat!(
            ":samp:`print({x} + 1)` :file:`/etc/{name}.conf` ",
            ":abbr:`LIFO (last-in, first-out)` :manpage:`ls(1)`\n",
        ),
        concat!(
            "document value=1\n  paragraph\n",
            "    role \"samp\"\n      text \"print(\"\n      emphasis\n        text \"x\"\n",
            "      text \" + 1)\"\n    text \" \"\n",
            "    role \"file\"\n      text \"/etc/\"\n      emphasis\n        text \"name\"\n",
            "      text \".conf\"\n    text \" \"\n",
            "    abbreviation @title=\"last-in, first-out\"\n      text \"LIFO\"\n",
            "    text \" \"\n",
            "    role \"manpage\"\n      text \"ls(1)\"\n",
        ),
        None,
    );
    let mut config = Config::default();
    config.manpages_url = "https://man.example/{section}/{page}".into();
    expect_contains(
        ":manpage:`ls(1)`\n",
        &["link link=external dest=\"https://man.example/1/ls\" @class=\"manpage\""],
        Some(&config),
    );
}

#[test]
fn pep_rfc_index_roles() {
    expect_dump(
        concat!(
            ":pep:`8` :rfc:`2822#section-3` :index:`spam` :index:`!eggs` ",
            ":index:`ham <pair: ham; eggs>`\n",
        ),
        concat!(
            "document value=1\n  paragraph\n",
            "    index @id=\"index-0\" @entries=\"single: Python Enhancement Proposals; PEP 8\"\n",
            "    link link=external dest=\"https://peps.python.org/pep-0008/\" @class=\"pep\"\n",
            "      strong\n        text \"PEP 8\"\n    text \" \"\n",
            "    index @id=\"index-1\" @entries=\"single: RFC; RFC 2822#section-3\"\n",
            "    link link=external dest=\"https://datatracker.ietf.org/doc/html/rfc2822.html",
            "#section-3\" @class=\"rfc\"\n",
            "      strong\n        text \"RFC 2822#section-3\"\n    text \" \"\n",
            "    index @id=\"index-2\" @entries=\"single: spam\"\n    text \"spam\"\n",
            "    text \" \"\n",
            "    index @id=\"index-3\" @entries=\"!single: eggs\"\n    text \"eggs\"\n",
            "    text \" \"\n",
            "    index @id=\"index-4\" @entries=\"pair: ham; eggs\"\n    text \"ham\"\n",
        ),
        None,
    );
    expect_codes(":pep:`eight`\n", "sphinx.role.pep", None);
}

#[test]
fn extlinks_roles() {
    let mut config = Config::default();
    config.extlinks = vec![
        crate::Extlink {
            name: "issue".into(),
            url: "https://example.org/issues/%s".into(),
            caption: "issue %s".into(),
        },
        crate::Extlink {
            name: "raw".into(),
            url: "https://example.org/%s".into(),
            caption: String::new(),
        },
    ];
    expect_dump(
        ":issue:`12` :issue:`the bug <34>` :raw:`x`\n",
        concat!(
            "document value=1\n  paragraph\n",
            "    link link=external dest=\"https://example.org/issues/12\" ",
            "@class=\"extlink-issue\"\n      text \"issue 12\"\n    text \" \"\n",
            "    link link=external dest=\"https://example.org/issues/34\" ",
            "@class=\"extlink-issue\"\n      text \"the bug\"\n    text \" \"\n",
            "    link link=external dest=\"https://example.org/x\" @class=\"extlink-raw\"\n",
            "      text \"https://example.org/x\"\n",
        ),
        Some(&config),
    );
}

#[test]
fn unknown_roles_are_still_errors() {
    expect_codes(":nosuch:`x`\n", "rst.role.unknown", None);
    expect_codes(":py:nosuch:`x`\n", "rst.role.unknown", None);
}

// sphinx.ext.intersphinx's :external: roles make references that resolve only through
// inventories; a role in the default domain is found first, then a standard one.
#[test]
fn external_roles() {
    let mut config = Config::default();
    config.intersphinx = true;
    config.intersphinx_resolve_self = "demo".into();
    expect_contains(
        concat!(
            ":external:py:class:`zipfile.ZipFile` ",
            ":external+python:ref:`tut-packages` :external:func:`len` ",
            ":external+demo:ref:`here`\n",
        ),
        &[
            "xref link=unresolved name=\"zipfile.ZipFile\" @reftype=\"py:class\" ",
            "@intersphinx=\"1\"",
            "xref link=unresolved name=\"tut-packages\" @reftype=\"ref\" ",
            "@intersphinx=\"1\" @inventory=\"python\"",
            "xref link=unresolved name=\"len\" @reftype=\"py:func\" @intersphinx=\"1\"",
            // The project's own name makes an ordinary reference.
            "xref link=unresolved name=\"here\" @reftype=\"ref\"\n",
        ],
        Some(&config),
    );
    expect_codes(
        ":external:a:b:c:`x` :external:nope:ref:`x` :external:py:nope:`x`\n",
        "sphinx.external.suffix sphinx.external.domain sphinx.external.role",
        Some(&config),
    );
    // Without the extension, the role is unknown, as in Sphinx.
    expect_codes(":external:py:class:`x`\n", "rst.role.unknown", None);
}
