//! Tests of the Python domain.

#[allow(unused_imports)]
use crate::config::{Config, TocParents};
#[allow(unused_imports)]
use crate::tests::*;

#[test]
fn python_function_signature() {
    expect_dump(
        concat!(
            ".. py:module:: mod\n\n",
            ".. py:function:: send(msg: str, *args, retries: int = 3, /, **kw) -> bool\n",
            "   :async:\n\n   Sends.\n",
        ),
        concat!(
            "document value=1\n",
            "  index @id=\"module-mod\" @entries=\"pair: module; mod\" @module=\"mod\"",
            " @domain=\"py\"\n",
            "  desc @domain=\"py\" @objtype=\"function\"\n",
            "    desc-signature @id=\"mod.send\" @fullname=\"send\" @module=\"mod\" ",
            "@toc=\"send()\"\n",
            "      desc-annotation\n        text \"async \"\n",
            "      desc-addname\n        text \"mod.\"\n",
            "      desc-name\n        text \"send\"\n",
            "      desc-parameterlist\n",
            "        desc-parameter\n          text \"msg\"\n          text \": \"\n",
            "          xref link=unresolved name=\"str\" @reftype=\"py:class\" ",
            "@py:module=\"mod\"\n            text \"str\"\n",
            "        desc-parameter\n          text \"*args\"\n",
            "        desc-parameter\n          text \"retries\"\n          text \": \"\n",
            "          xref link=unresolved name=\"int\" @reftype=\"py:class\" ",
            "@py:module=\"mod\"\n            text \"int\"\n",
            "          text \" = \"\n          text \"3\"\n",
            "        desc-parameter\n          text \"/\"\n",
            "        desc-parameter\n          text \"**kw\"\n",
            "      desc-returns\n",
            "        xref link=unresolved name=\"bool\" @reftype=\"py:class\" ",
            "@py:module=\"mod\"\n          text \"bool\"\n",
            "    index @entries=\"single: send() (in module mod)\" @targets=\"mod.send\"\n",
            "    desc-content\n      paragraph\n        text \"Sends.\"\n",
        ),
        None,
    );
}

#[test]
fn python_overloads_share_one_target() {
    // Only the first signature of a name gets the id and the index entry.
    expect_dump(
        ".. py:function:: f(x: int) -> int\n                 f(x: str) -> str\n",
        concat!(
            "document value=1\n",
            "  desc @domain=\"py\" @objtype=\"function\"\n",
            "    desc-signature @id=\"f\" @fullname=\"f\" @toc=\"f()\"\n",
            "      desc-name\n        text \"f\"\n",
            "      desc-parameterlist\n",
            "        desc-parameter\n          text \"x\"\n          text \": \"\n",
            "          xref link=unresolved name=\"int\" @reftype=\"py:class\"\n",
            "            text \"int\"\n",
            "      desc-returns\n",
            "        xref link=unresolved name=\"int\" @reftype=\"py:class\"\n",
            "          text \"int\"\n",
            "    desc-signature @fullname=\"f\" @toc=\"f()\"\n",
            "      desc-name\n        text \"f\"\n",
            "      desc-parameterlist\n",
            "        desc-parameter\n          text \"x\"\n          text \": \"\n",
            "          xref link=unresolved name=\"str\" @reftype=\"py:class\"\n",
            "            text \"str\"\n",
            "      desc-returns\n",
            "        xref link=unresolved name=\"str\" @reftype=\"py:class\"\n",
            "          text \"str\"\n",
            "    index @entries=\"pair: built-in function; f()\" @targets=\"f\"\n",
            "    desc-content\n",
        ),
        None,
    );
}

#[test]
fn python_class_nesting() {
    expect_contains(
        concat!(
            ".. py:class:: Queue(maxsize=0)\n   :final:\n\n",
            "   .. py:method:: put(item, block=True)\n      :classmethod:\n\n",
            "   .. py:attribute:: size\n      :type: Optional[int]\n      :value: 0\n\n",
            "   .. py:property:: full\n\n",
            ".. py:staticmethod:: Other.make()\n\n",
            "   Uses :meth:`put`.\n",
        ),
        &[
            "desc @domain=\"py\" @objtype=\"class\"",
            "desc-signature @id=\"Queue\" @fullname=\"Queue\" @toc=\"Queue\"",
            "desc-annotation\n        text \"final class \"",
            concat!(
                "desc-parameter\n          text \"maxsize\"\n          text \"=\"\n",
                "          text \"0\"",
            ),
            "index @entries=\"single: Queue (built-in class)\"",
            concat!(
                "desc-signature @id=\"Queue.put\" @fullname=\"Queue.put\" @toc=\"Queue.put()\" ",
                "@class=\"Queue\"",
            ),
            "text \"classmethod \"",
            "single: put() (Queue class method)",
            concat!(
                "desc-signature @id=\"Queue.size\" @fullname=\"Queue.size\" @toc=\"Queue.size\" ",
                "@class=\"Queue\"",
            ),
            "desc-annotation\n            text \": \"",
            "name=\"Optional\"",
            "name=\"int\"",
            "text \" = 0\"",
            "single: size (Queue attribute)",
            "@id=\"Queue.full\"",
            "text \"property \"",
            "single: full (Queue property)",
            concat!(
                "desc-signature @id=\"Other.make\" @fullname=\"Other.make\" @toc=\"Other.make\" ",
                "@class=\"Other\"",
            ),
            "text \"static \"",
            "desc-addname\n        text \"Other.\"",
            "single: make() (Other static method)",
            "@reftype=\"py:meth\" @py:class=\"Other\"",
        ],
        None,
    );
}

#[test]
fn python_signature_fallbacks() {
    expect_dump(
        concat!(
            ".. py:function:: compile(source[, filename[, mode]])\n\n",
            ".. py:function:: not a signature!\n",
        ),
        concat!(
            "document value=1\n",
            "  desc @domain=\"py\" @objtype=\"function\"\n",
            "    desc-signature @id=\"compile\" @fullname=\"compile\" @toc=\"compile()\"\n",
            "      desc-name\n        text \"compile\"\n",
            "      desc-parameterlist\n",
            "        desc-parameter\n          text \"source\"\n",
            "        desc-optional\n",
            "          desc-parameter\n            text \"filename\"\n",
            "          desc-optional\n",
            "            desc-parameter\n              text \"mode\"\n",
            "    index @entries=\"pair: built-in function; compile()\" @targets=\"compile\"\n",
            "    desc-content\n",
            "  desc @domain=\"py\" @objtype=\"function\"\n",
            "    desc-signature\n      desc-name\n        text \"not a signature!\"\n",
            "    desc-content\n",
        ),
        None,
    );
}

#[test]
fn python_other_objects() {
    expect_contains(
        concat!(
            ".. py:currentmodule:: m\n\n",
            ".. py:data:: VERSION\n   :type: str\n\n",
            ".. py:exception:: Oops\n\n",
            ".. py:decorator:: cached(size)\n\n",
            ".. py:type:: Vector\n   :value: list[float]\n\n",
            ".. py:function:: a()\n              b()\n   :no-index:\n",
        ),
        &[
            "@id=\"m.VERSION\"",
            "single: VERSION (in module m)",
            "@objtype=\"exception\"",
            "text \"exception \"",
            "single: Oops",
            "@id=\"m.cached\"",
            "desc-addname\n        text \"@\"",
            "text \"type \"",
            "text \" = list[float]\"",
            "single: Vector (type alias in m)",
            "desc @domain=\"py\" @objtype=\"function\" @noindex=\"1\"",
            "desc-signature @fullname=\"a\" @module=\"m\"",
            "desc-signature @fullname=\"b\" @module=\"m\"",
        ],
        None,
    );
}

#[test]
fn python_module_options_and_add_module_names() {
    let mut config = Config::default();
    config.add_module_names = false;
    expect_dump(
        ".. py:function:: f()\n   :module: other\n",
        concat!(
            "document value=1\n",
            "  desc @domain=\"py\" @objtype=\"function\"\n",
            "    desc-signature @id=\"other.f\" @fullname=\"f\" @module=\"other\" @toc=\"f()\"\n",
            "      desc-name\n        text \"f\"\n",
            "      desc-parameterlist\n",
            "    index @entries=\"single: f() (in module other)\" @targets=\"other.f\"\n",
            "    desc-content\n",
        ),
        Some(&config),
    );
}

// Sphinx's tests/test_domains/test_domain_py_canonical.py: the canonical name is noted
// on the signature for the project to register as an alias; a py:type shows its
// canonical type instead, and objects without index entries note none.
#[test]
fn python_canonical_names() {
    expect_contains(
        concat!(
            ".. py:class:: io.StringIO\n   :canonical: _io.StringIO\n\n",
            ".. py:type:: Alias\n   :canonical: list[int]\n\n",
            ".. py:function:: hidden()\n   :canonical: _impl.hidden\n   :no-index:\n",
        ),
        &[
            concat!(
                "desc-signature @id=\"io.StringIO\" @fullname=\"io.StringIO\" ",
                "@toc=\"io.StringIO\" @class=\"io\" ",
                "@canonical=\"_io.StringIO\"\n",
            ),
            "desc-signature @id=\"Alias\" @fullname=\"Alias\" @toc=\"Alias\"\n",
            concat!(
                "desc-name\n        text \"Alias\"\n      desc-annotation\n        text \" = \"\n",
                "        xref link=unresolved name=\"list\" @reftype=\"py:class\"\n",
                "          text \"list\"\n        text \"[\"\n",
                "        xref link=unresolved name=\"int\" @reftype=\"py:class\"\n",
            ),
            "desc-signature @fullname=\"hidden\"\n",
        ],
        None,
    );
}
