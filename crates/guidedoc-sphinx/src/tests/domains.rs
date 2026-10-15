//! Tests of the object domains: signatures, ids, index entries and nesting.

#[allow(unused_imports)]
use crate::config::{Config, TocParents};
#[allow(unused_imports)]
use crate::tests::*;

#[test]
fn javascript_objects() {
    expect_dump(
        concat!(
            ".. js:module:: lib\n\n",
            ".. js:class:: Widget(el)\n\n",
            "   .. js:method:: render(opts, [cb])\n",
        ),
        concat!(
            "document value=1\n",
            "  index @id=\"module-lib\" @entries=\"single: lib (module)\" @module=\"lib\"",
            " @domain=\"js\"\n",
            "  desc @domain=\"js\" @objtype=\"class\"\n",
            "    desc-signature @id=\"lib.Widget\" @fullname=\"Widget\" @module=\"lib\" ",
            "@toc=\"Widget\"\n",
            "      desc-annotation\n        text \"class \"\n",
            "      desc-addname\n        text \"lib.\"\n",
            "      desc-name\n        text \"Widget\"\n",
            "      desc-parameterlist\n        desc-parameter\n          text \"el\"\n",
            "    index @entries=\"single: Widget() (class)\" @targets=\"lib.Widget\"\n",
            "    desc-content\n",
            "      desc @domain=\"js\" @objtype=\"method\"\n",
            "        desc-signature @id=\"lib.Widget.render\" @fullname=\"Widget.render\" ",
            "@module=\"lib\" @toc=\"Widget.render()\" @object=\"Widget\"\n",
            "          desc-addname\n            text \"Widget.\"\n",
            "          desc-name\n            text \"render\"\n",
            "          desc-parameterlist\n",
            "            desc-parameter\n              text \"opts\"\n",
            "            desc-optional\n              desc-parameter\n",
            "                text \"cb\"\n",
            "        index @entries=\"single: render() (Widget method)\" ",
            "@targets=\"lib.Widget.render\"\n",
            "        desc-content\n",
        ),
        None,
    );
}

// A C function notes its named parameters (Sphinx's functionParam symbols), and a
// member may be written with its struct ("float Sphinx.version").
#[test]
fn c_parameters_and_qualified_members() {
    expect_contains(
        concat!(
            ".. c:function:: int f(const char *name, int values[4], int (*cb)(int), void *)\n\n",
            ".. c:function:: void g(void)\n\n",
            ".. c:member:: float Sphinx.version\n",
        ),
        &[
            "@id=\"c.f\" @fullname=\"f\" @toc=\"f()\" @params=\"name\\nvalues\\ncb\"",
            "@id=\"c.g\" @fullname=\"g\" @toc=\"g()\"\n",
            "@id=\"c.Sphinx.version\" @fullname=\"Sphinx.version\" @toc=\"version\"",
            "desc-addname\n        xref link=unresolved name=\"Sphinx\" @reftype=\"c:identifier\"",
            "          text \"Sphinx\"\n        text \".\"\n      desc-name",
        ],
        None,
    );
}

// Anonymous entities ("@data") keep their names in ids and full names, and show as
// "[anonymous]" (Sphinx's test-domain-c).
#[test]
fn c_anonymous_entities() {
    expect_contains(
        concat!(
            ".. c:struct:: A\n\n   .. c:union:: @data\n\n      .. c:member:: int a\n\n",
            ".. c:struct:: @b.B\n",
        ),
        &[
            "@id=\"c.A.@data\" @fullname=\"A.@data\"",
            "desc-name\n            text \"[anonymous]\"",
            "@id=\"c.A.@data.a\" @fullname=\"A.@data.a\"",
            "@id=\"c.@b.B\" @fullname=\"@b.B\"",
            "desc-addname\n        text \"[anonymous].\"",
        ],
        None,
    );
}

#[test]
fn c_declarations() {
    expect_contains(
        concat!(
            ".. c:function:: int printf(const char *fmt, ...)\n\n",
            ".. c:macro:: MAX(a, b)\n\n",
            ".. c:struct:: point\n\n",
            "   .. c:member:: int x\n\n",
            ".. c:type:: size_type\n\n",
            ".. c:var:: int counter = 0\n",
        ),
        &[
            "desc-signature @id=\"c.printf\" @fullname=\"printf\"",
            "text \"int \"",
            "desc-name\n        text \"printf\"",
            "desc-parameter\n          text \"const char *fmt\"",
            "desc-parameter\n          text \"...\"",
            "single: printf (C function)",
            "@id=\"c.MAX\"",
            "single: MAX (C macro)",
            "@id=\"c.point\"",
            "text \"struct \"",
            "@id=\"c.point.x\" @fullname=\"point.x\"",
            "single: point.x (C member)",
            "@id=\"c.size_type\"",
            "text \"type \"",
            "@id=\"c.counter\"",
            "text \" = 0\"",
            "single: counter (C member)",
        ],
        None,
    );
}

#[test]
fn cpp_declarations_and_ids() {
    expect_contains(
        concat!(
            ".. cpp:class:: Foo : public Bar\n\n",
            "   .. cpp:function:: void f(int i) const\n\n",
            "   .. cpp:member:: std::string name\n\n",
            ".. cpp:namespace-push:: ns\n\n",
            ".. cpp:function:: const std::string &get(double d, char *p)\n\n",
            ".. cpp:function:: template<typename T> void apply(T t)\n\n",
            ".. cpp:namespace-pop::\n\n",
            ".. cpp:function:: void none()\n",
        ),
        &[
            concat!(
                "desc-signature @id=\"_CPPv43Foo\" @id=\"_CPPv33Foo\" @id=\"_CPPv23Foo\" ",
                "@id=\"Foo\" @fullname=\"Foo\"",
            ),
            "text \"class \"",
            "text \" : public \"",
            "xref link=unresolved name=\"Bar\" @reftype=\"cpp:identifier\"",
            "@id=\"_CPPv4NK3Foo1fEi\"",
            "@id=\"Foo::f__iC\" @fullname=\"Foo::f\"",
            "text \"(int i) const\"",
            "@id=\"Foo::name__ss\" @fullname=\"Foo::name\"",
            "@id=\"ns::get__double.cP\" @fullname=\"ns::get\"",
            "@id=\"_CPPv4I0EN2ns5applyEv1T\" @id=\"_CPPv3I0EN2ns5applyE1T\"",
            "@cpp:tparams=\"T\" @cpp:fparams=\"t\"",
            "@id=\"_CPPv44nonev\"",
        ],
        None,
    );
}

// Every id version Sphinx gives, for operators, anonymous entities, templates, and
// visibility (the ids Sphinx 9.1 writes for its test-domain-cpp).
#[test]
fn cpp_ids_of_every_version() {
    expect_contains(
        concat!(
            ".. cpp:function:: void paren_5::operator()(int)\n\n",
            ".. cpp:function:: int operator+(int, int)\n\n",
            ".. cpp:function:: int operator\"\"_lit()\n\n",
            ".. cpp:class:: B\n\n   .. cpp:function:: operator int()\n\n",
            ".. cpp:var:: char c = '\\\\'\n\n",
            ".. cpp:enum-struct:: protected E : std::underlying_type<S>::type\n\n",
            ".. cpp:namespace:: ns\n\n",
            ".. cpp:class:: @a::A\n\n",
            ".. cpp:class:: template<typename T, int V, template<typename> typename U> C\n",
        ),
        &[
            concat!(
                "@id=\"_CPPv4N7paren_5clEi\" @id=\"_CPPv3N7paren_5clEi\" ",
                "@id=\"_CPPv2N7paren_5clEi\" @id=\"paren_5::call-operator__i\"",
            ),
            concat!(
                "@id=\"_CPPv4plii\" @id=\"_CPPv3plii\" @id=\"_CPPv2plii\" ",
                "@id=\"add-operator__i.i\"",
            ),
            concat!(
                "@id=\"_CPPv4li4_litv\" @id=\"_CPPv3li4_litv\" @id=\"_CPPv2li4_litv\" ",
                "@fullname=\"operator\\\"\\\"_lit\"",
            ),
            concat!(
                "@id=\"_CPPv4N1BcviEv\" @id=\"_CPPv3N1BcviEv\" @id=\"_CPPv2N1BcviEv\" ",
                "@id=\"B::castto-i-operator\"",
            ),
            "@id=\"_CPPv41c\" @id=\"_CPPv31c\" @id=\"_CPPv21c\" @id=\"c__c\"",
            "@id=\"_CPPv41E\" @id=\"_CPPv31E\" @id=\"_CPPv21E\" @fullname=\"E\"",
            concat!(
                "@id=\"_CPPv4N2nsUt1_a1AE\" @id=\"_CPPv3N2nsUt1_a1AE\" ",
                "@fullname=\"ns::@a::A\" @cpp:display=\"ns::[anonymous]::A\"",
            ),
            concat!(
                "@id=\"_CPPv4I0_iI0E0EN2ns1CE\" @id=\"_CPPv3I0_iI0E0EN2ns1CE\" ",
                "@id=\"_CPPv2I0_iI0E0EN2ns1CE\" @fullname=\"ns::C\"",
            ),
            "@cpp:tparams=\"T\\nV\\nU\"",
        ],
        None,
    );
}

#[test]
fn rst_domain_objects() {
    expect_dump(
        concat!(
            ".. rst:directive:: .. toctree:: [entries]\n\n",
            "   .. rst:directive:option:: maxdepth: number\n      :type: int\n\n",
            ".. rst:role:: ref\n",
        ),
        concat!(
            "document value=1\n",
            "  desc @domain=\"rst\" @objtype=\"directive\"\n",
            "    desc-signature @id=\"directive-toctree\" @fullname=\"toctree\" ",
            "@toc=\".. toctree::\"\n",
            "      desc-name\n        text \".. toctree::\"\n",
            "      desc-addname\n        text \" [entries]\"\n",
            "    index @entries=\"single: toctree (directive)\" @targets=\"directive-toctree\"\n",
            "    desc-content\n",
            "      desc @domain=\"rst\" @objtype=\"directive:option\"\n",
            "        desc-signature @id=\"directive-option-toctree-maxdepth\" ",
            "@fullname=\"maxdepth\" @toc=\":maxdepth:\"\n",
            "          desc-name\n            text \":maxdepth:\"\n",
            "          desc-annotation\n            text \" number\"\n",
            "          desc-annotation\n            text \" (int)\"\n",
            "        index @entries=\"pair: toctree (directive); :maxdepth: (directive option)\" ",
            "@targets=\"directive-option-toctree-maxdepth\"\n",
            "        desc-content\n",
            "  desc @domain=\"rst\" @objtype=\"role\"\n",
            "    desc-signature @id=\"role-ref\" @fullname=\"ref\" @toc=\":ref:\"\n",
            "      desc-name\n        text \":ref:\"\n",
            "    index @entries=\"single: ref (role)\" @targets=\"role-ref\"\n",
            "    desc-content\n",
        ),
        None,
    );
}

#[test]
fn std_domain_objects() {
    expect_dump(
        concat!(
            ".. program:: rm\n\n",
            ".. option:: -r, --recursive, --out=FILE\n\n",
            ".. envvar:: HOME\n\n",
            ".. confval:: timeout\n   :type: ``int``\n   :default: ``30``\n\n",
            ".. describe:: anything goes\n",
        ),
        concat!(
            "document value=1\n",
            "  desc @domain=\"std\" @objtype=\"option\"\n",
            "    desc-signature @id=\"cmdoption-rm-r\" @id=\"cmdoption-rm-recursive\" ",
            "@id=\"cmdoption-rm-out\" @fullname=\"-r\" @program=\"rm\" ",
            "@options=\"-r\\n--recursive\\n--out\"\n",
            "      desc-name\n        text \"-r\"\n",
            "      desc-addname\n        text \", \"\n",
            "      desc-name\n        text \"--recursive\"\n",
            "      desc-addname\n        text \", \"\n",
            "      desc-name\n        text \"--out\"\n",
            "      desc-addname\n        text \"=FILE\"\n",
            "    index @entries=\"pair: rm command line option; -r\\npair: rm command line ",
            "option; --recursive\\npair: rm command line option; --out\" ",
            "@targets=\"cmdoption-rm-r\\ncmdoption-rm-r\\ncmdoption-rm-r\"\n",
            "    desc-content\n",
            "  desc @domain=\"std\" @objtype=\"envvar\"\n",
            "    desc-signature @id=\"envvar-HOME\" @fullname=\"HOME\"\n",
            "      desc-name\n        text \"HOME\"\n",
            "    index @entries=\"single: environment variable; HOME\" @targets=\"envvar-HOME\"\n",
            "    desc-content\n",
            "  desc @domain=\"std\" @objtype=\"confval\"\n",
            "    desc-signature @id=\"confval-timeout\" @fullname=\"timeout\" @toc=\"timeout\"\n",
            "      desc-name\n        text \"timeout\"\n",
            "    index @entries=\"pair: timeout; configuration value\" ",
            "@targets=\"confval-timeout\"\n",
            "    desc-content\n",
            "      field-list\n",
            "        field\n          field-name\n            text \"Type\"\n",
            "          field-body\n            paragraph\n              code \"int\"\n",
            "        field\n          field-name\n            text \"Default\"\n",
            "          field-body\n            paragraph\n              code \"30\"\n",
            "  desc @domain=\"std\" @objtype=\"describe\"\n",
            "    desc-signature\n      desc-name\n        text \"anything goes\"\n",
            "    desc-content\n",
        ),
        None,
    );
}

#[test]
fn default_domain_directives() {
    expect_contains(
        concat!(
            ".. default-domain:: js\n\n.. function:: go()\n\n.. envvar:: X\n\n",
            ".. default-domain:: py\n\n.. class:: K\n",
        ),
        &[
            "@domain=\"js\" @objtype=\"function\"",
            "@domain=\"std\" @objtype=\"envvar\"",
            "@domain=\"py\" @objtype=\"class\"",
        ],
        None,
    );
}

#[test]
fn object_options_and_duplicates() {
    expect_contains(
        concat!(
            ".. py:function:: f()\n   :no-index-entry:\n   :no-contents-entry:\n\n",
            ".. py:function:: f()\n",
        ),
        &[
            "desc @domain=\"py\" @objtype=\"function\" @no-contents-entry=\"1\"",
            "desc-signature @id=\"f\"",
            "desc-content",
            "desc-signature @id=\"id0\"",
            "index @entries=\"pair: built-in function; f()\"",
        ],
        None,
    );
}

#[test]
fn alias_and_namespace() {
    expect_dump(
        concat!(
            ".. c:namespace:: lib\n\n.. c:function:: void run(void)\n\n",
            ".. cpp:alias:: Foo\n   Bar\n   :maxdepth: 2\n",
        ),
        concat!(
            "document value=1\n",
            "  desc @domain=\"c\" @objtype=\"function\"\n",
            "    desc-signature @id=\"c.lib.run\" @fullname=\"lib.run\" @toc=\"run()\"\n",
            "      text \"void \"\n",
            "      desc-name\n        text \"run\"\n",
            "      desc-parameterlist\n        desc-parameter\n          text \"void\"\n",
            "    index @entries=\"single: lib.run (C function)\" @targets=\"c.lib.run\"\n",
            "    desc-content\n",
            "  directive @name=\"cpp:alias\" @names=\"Foo\\nBar\" @maxdepth=\"2\"\n",
        ),
        None,
    );
}

#[test]
fn c_parenthesized_declarators() {
    expect_dump(
        concat!(
            ".. c:type:: int (*visitproc)(PyObject *object, void *arg)\n\n",
            ".. c:struct:: S\n\n",
            "   .. c:member:: Py_hash_t (*const hash)(const void *, Py_ssize_t)\n\n",
            "   .. c:member:: PyObject* (*initfunc)(void)\n",
        ),
        concat!(
            "document value=1\n",
            "  desc @domain=\"c\" @objtype=\"type\"\n",
            "    desc-signature @id=\"c.visitproc\" @fullname=\"visitproc\" @toc=\"visitproc\"\n",
            "      desc-annotation\n        text \"typedef \"\n",
            "      text \"int (*\"\n",
            "      desc-name\n        text \"visitproc\"\n",
            "      text \")\"\n",
            "      desc-parameterlist\n",
            "        desc-parameter\n",
            "          xref link=unresolved name=\"PyObject\" @reftype=\"c:identifier\" ",
            "@unstyled=\"1\" @c:parent=\"visitproc\"\n            text \"PyObject\"\n",
            "          text \" *object\"\n",
            "        desc-parameter\n          text \"void *arg\"\n",
            "    index @entries=\"single: visitproc (C type)\" @targets=\"c.visitproc\"\n",
            "    desc-content\n",
            "  desc @domain=\"c\" @objtype=\"struct\"\n",
            "    desc-signature @id=\"c.S\" @fullname=\"S\" @toc=\"S\"\n",
            "      desc-annotation\n        text \"struct \"\n",
            "      desc-name\n        text \"S\"\n",
            "    index @entries=\"single: S (C struct)\" @targets=\"c.S\"\n",
            "    desc-content\n",
            "      desc @domain=\"c\" @objtype=\"member\"\n",
            "        desc-signature @id=\"c.S.hash\" @fullname=\"S.hash\" @toc=\"hash\"\n",
            "          xref link=unresolved name=\"Py_hash_t\" @reftype=\"c:identifier\" ",
            "@unstyled=\"1\" @c:parent=\"S.hash\"\n            text \"Py_hash_t\"\n",
            "          text \" (*const \"\n",
            "          desc-name\n            text \"hash\"\n",
            "          text \")\"\n",
            "          desc-parameterlist\n",
            "            desc-parameter\n              text \"const void *\"\n",
            "            desc-parameter\n",
            "              xref link=unresolved name=\"Py_ssize_t\" @reftype=\"c:identifier\" ",
            "@unstyled=\"1\" @c:parent=\"S.hash\"\n                text \"Py_ssize_t\"\n",
            "        index @entries=\"single: S.hash (C member)\" @targets=\"c.S.hash\"\n",
            "        desc-content\n",
            "      desc @domain=\"c\" @objtype=\"member\"\n",
            "        desc-signature @id=\"c.S.initfunc\" @fullname=\"S.initfunc\" ",
            "@toc=\"initfunc\"\n",
            "          xref link=unresolved name=\"PyObject\" @reftype=\"c:identifier\" ",
            "@unstyled=\"1\" @c:parent=\"S.initfunc\"\n            text \"PyObject\"\n",
            "          text \"* (*\"\n",
            "          desc-name\n            text \"initfunc\"\n",
            "          text \")\"\n",
            "          desc-parameterlist\n",
            "            desc-parameter\n              text \"void\"\n",
            "        index @entries=\"single: S.initfunc (C member)\" @targets=\"c.S.initfunc\"\n",
            "        desc-content\n",
        ),
        None,
    );
}

#[test]
fn c_type_prefix() {
    expect_contains(
        concat!(
            ".. c:type:: PyObject\n\n",
            ".. c:type:: PyObject *(*vectorcallfunc)(PyObject *callable, ",
            "PyObject *const *args, size_t nargsf, PyObject *kwnames)\n\n",
            ".. c:type:: int (*table)[4]\n",
        ),
        &[
            "@id=\"c.PyObject\"",
            "text \"type \"",
            "@id=\"c.vectorcallfunc\"",
            "text \"typedef \"",
            "text \" *(*\"",
            concat!(
                "desc-parameter\n          xref link=unresolved name=\"PyObject\" ",
                "@reftype=\"c:identifier\" @unstyled=\"1\" @c:parent=\"vectorcallfunc\"\n",
                "            text \"PyObject\"\n          text \" *const *args\"",
            ),
            "@id=\"c.table\"",
            "desc-name\n        text \"table\"\n      text \")[4]\"",
        ],
        None,
    );
}

#[test]
fn option_names_follow_sphinx_pattern() {
    // The name is the leading run without white space or "=": "-" (standard input) and
    // "--" are names; a spelling that starts with "=" is malformed.
    expect_contains(
        concat!(
            ".. program:: python\n\n",
            ".. option:: -\n\n",
            ".. option:: --, -o FILE, --level[=N]\n",
        ),
        &[
            "desc-signature @id=\"cmdoption-python-0\" @fullname=\"-\" @program=\"python\"",
            "desc-name\n        text \"-\"\n",
            "index @entries=\"pair: python command line option; -\"",
            "@fullname=\"--\"",
            "@options=\"--\\n-o\\n--level\"",
            "desc-name\n        text \"-o\"\n      desc-addname\n        text \" FILE\"",
            "desc-name\n        text \"--level\"\n      desc-addname\n        text \"[=N]\"",
        ],
        None,
    );
    let r = read_text(".. option:: -\n\n.. option:: =x\n", None);
    assert!(!r.dump.contains("@fullname=\"=x\""), "=x is not an option");
    assert_eq!(r.codes.matches("sphinx.option.malformed").count(), 1);
}
