//! Ported from the Odin package's docfields_test.odin.

#[allow(unused_imports)]
use crate::config::{Config, TocParents};
#[allow(unused_imports)]
use crate::tests::*;

const XREF_CONTEXT: &str =
    concat!(" @refspecific=\"1\" @py:module=\"example\" @py:class=\"Class\"", " @unstyled=\"1\"",);

#[test]
fn info_field_list() {
    let first = format!(
        concat!(
            "    desc-content\n      field-list\n        field @docfield=\"parameter\"\n",
            "          field-name +generated\n",
            "            text +generated \"Parameters\"\n          field-body\n",
            "            list list=bullet,*,1\n              list-item\n",
            "                paragraph\n",
            "                  strong +generated @literal=\"1\"\n",
            "                    text +generated \"name\"\n",
            "                  text +generated \" (\"\n",
            "                  xref +generated link=unresolved name=\"str\" ",
            "@reftype=\"py:class\"{}\n",
            "                    emphasis +generated @literal=\"1\"\n",
            "                      text +generated \"str\"\n",
            "                  text +generated \")\"\n",
            "                  text +generated \" – \"\n",
            "                  text \"blah blah\"\n",
        ),
        XREF_CONTEXT
    );
    expect_contains(
        concat!(
            ".. py:module:: example\n.. py:class:: Class\n\n",
            "   :meta blah: this meta-field must not show up in the toc-tree\n",
            "   :param str name: blah blah\n   :meta another meta field:\n",
            "   :param age: blah blah\n   :type age: int\n",
            "   :param items: blah blah\n   :type items: Tuple[str, ...]\n",
            "   :param Dict[str, str] params: blah blah\n",
        ),
        &[
            &first,
            "text +generated \"age\"",
            "name=\"int\"",
            "text \"blah blah\"",
            "text +generated \"items\"",
            "name=\"Tuple\"",
            "emphasis +generated @literal=\"1\"\n                    text +generated \"[\"",
            "name=\"str\"",
            "text +generated \", \"",
            "text +generated \"...\"",
            "text +generated \"]\"",
            "text +generated \"params\"",
            "name=\"Dict\"",
            "name=\"str\"",
            "text +generated \", \"",
            "name=\"str\"",
            "text +generated \"]\"",
        ],
        None,
    );
    expect_dump(
        ".. py:class:: Class\n\n   :meta blah: x\n   :meta: y\n",
        concat!(
            "document value=1\n  desc @domain=\"py\" @objtype=\"class\"\n",
            "    desc-signature @id=\"Class\" @fullname=\"Class\" @toc=\"Class\"\n",
            "      desc-annotation\n        text \"class \"\n      desc-name\n",
            "        text \"Class\"\n    index @entries=\"single: Class (built-in class)\" ",
            "@targets=\"Class\"\n    desc-content\n      field-list\n",
        ),
        None,
    );
}

#[test]
fn info_field_list_piped_type() {
    let want = format!(
        concat!(
            "      field-list\n        field @docfield=\"parameter\"\n",
            "          field-name +generated\n",
            "            text +generated \"Parameters\"\n",
            "          field-body\n            paragraph\n",
            "              strong +generated @literal=\"1\"\n",
            "                text +generated \"age\"\n",
            "              text +generated \" (\"\n",
            "              xref +generated link=unresolved name=\"int\" ",
            "@reftype=\"py:class\"{ctx}\n",
            "                emphasis +generated @literal=\"1\"\n",
            "                  text +generated \"int\"\n",
            "              emphasis +generated @literal=\"1\"\n",
            "                text +generated \" | \"\n",
            "              xref +generated link=unresolved name=\"str\" @reftype=\"py:class\"",
            "{ctx}\n                emphasis +generated @literal=\"1\"\n",
            "                  text +generated \"str\"\n",
            "              text +generated \")\"\n",
            "              text +generated \" – \"\n              text \"blah blah\"\n",
        ),
        ctx = XREF_CONTEXT
    );
    expect_contains(
        concat!(
            ".. py:module:: example\n.. py:class:: Class\n\n",
            "   :param age: blah blah\n   :type age: int | str\n",
        ),
        &[&want],
        None,
    );
}

// After a Literal, the parts are shown unlinked, and smart quotes leave them straight.
#[test]
fn info_field_list_literal() {
    expect_contains(
        concat!(
            ".. py:module:: example\n.. py:class:: Class\n\n",
            "   :param age: blah blah\n   :type age: Literal['foo', 'bar', 'baz']\n",
        ),
        &[
            "xref +generated link=unresolved name=\"Literal\"",
            "text +generated \"Literal\"",
            "emphasis +generated @literal=\"1\"\n                text +generated \"[\"\n",
            "emphasis +generated @literal=\"1\"\n                text +generated \"'foo'\"\n",
            "emphasis +generated @literal=\"1\"\n                text +generated \", \"\n",
            "emphasis +generated @literal=\"1\"\n                text +generated \"'bar'\"\n",
            "emphasis +generated @literal=\"1\"\n                text +generated \", \"\n",
            "emphasis +generated @literal=\"1\"\n                text +generated \"'baz'\"\n",
            "emphasis +generated @literal=\"1\"\n                text +generated \"]\"\n",
            "text +generated \")\"\n              text +generated \" – \"\n",
        ],
        None,
    );
}

#[test]
fn info_field_list_var_and_delimiters() {
    expect_contains(
        ".. py:class:: Class\n\n   :var int attr: blah blah\n",
        &[concat!(
            "field @docfield=\"variable\"\n          field-name +generated\n",
            "            text +generated \"Variables\"\n          field-body\n",
            "            paragraph\n              strong +generated @literal=\"1\"\n",
            "                text +generated \"attr\"\n              text +generated \" (\"\n",
            "              xref +generated link=unresolved name=\"int\" @reftype=\"py:class\" ",
            "@refspecific=\"1\" @py:class=\"Class\" @unstyled=\"1\"\n",
        )],
        None,
    );
    expect_contains(
        concat!(
            ".. py:module:: example\n.. py:class:: Class\n\n",
            "   :param list_str_var: example description.\n",
            "   :type list_str_var: list of str\n",
            "   :param bool_str_var: example description.\n",
            "   :type bool_str_var: str or float or int\n",
        ),
        &[
            "name=\"list\"",
            "text +generated \" of \"",
            "name=\"str\"",
            "text \"example description.\"",
            "name=\"str\"",
            "text +generated \" or \"",
            "name=\"float\"",
            "text +generated \" or \"",
            "name=\"int\"",
        ],
        None,
    );
}

// Returns, return types, and exceptions: "~" and "typing." change what is shown and make
// the reference py:class or py:obj; None keeps the field's role.
#[test]
fn info_field_list_returns_and_raises() {
    expect_contains(
        concat!(
            ".. py:function:: f()\n\n   :returns: a new :class:`Time` instance\n",
            "   :rtype: ~a.B | None\n   :raises ~errors.Bad: when\n",
            "   :raises typing.Never: never\n",
        ),
        &[
            "field @docfield=\"returnvalue\"",
            "text +generated \"Returns\"",
            "paragraph\n              text \"a new \"\n              xref",
            "text \" instance\"",
            "field @docfield=\"returntype\"",
            "text +generated \"Return type\"",
            concat!(
                "xref +generated link=unresolved name=\"a.B\" @reftype=\"py:class\" ",
                "@refspecific=\"1\" @unstyled=\"1\"\n                emphasis +generated\n",
                "                  text +generated \"B\"",
            ),
            "text +generated \" | \"",
            "xref +generated link=unresolved name=\"None\" @reftype=\"py:class\"",
            "text +generated \"None\"",
            "field @docfield=\"exceptions\"",
            "text +generated \"Raises\"",
            "list list=bullet",
            "name=\"errors.Bad\" @reftype=\"py:class\"",
            "strong +generated @literal=\"1\"\n                      text +generated \"Bad\"",
            "text +generated \" – \"",
            "text \"when\"",
            "name=\"typing.Never\" @reftype=\"py:obj\"",
            "text +generated \"Never\"",
        ],
        None,
    );
}

// Unknown fields and fields with a wrong argument keep their place with the name
// capitalized; a type field without an argument links its text. Objects of other
// domains without field types capitalize every field.
#[test]
fn info_field_list_unknown_fields() {
    expect_contains(
        concat!(
            ".. py:function:: func_with_unknown_field()\n\n   :field_name:\n\n",
            "   :field_name all lower:\n\n   :FIELD_NAME ALL CAPS:\n\n   :param:\n\n",
            "   :returns x: y\n\n   :type: int\n\n.. describe:: thing\n\n   :note: n\n",
        ),
        &[
            "field\n          field-name\n            text \"Field_name\"",
            "text \"Field_name all lower\"",
            "text \"FIELD_NAME ALL CAPS\"",
            "text \"Param\"",
            "text \"Returns x\"",
            concat!(
                "text \"Type\"\n          field-body\n            paragraph\n",
                "              xref link=unresolved name=\"int\" @reftype=\"py:class\" ",
                "@refspecific=\"1\" @unstyled=\"1\"\n                text \"int\"",
            ),
            "desc @domain=\"std\"",
            "text \"Note\"",
        ],
        None,
    );
}

// Content of several blocks follows the use's paragraph; a typed use without text has
// no separator; a group of one of a type that does not collapse is still a list.
#[test]
fn info_field_list_blocks_and_empty() {
    expect_contains(
        concat!(
            ".. py:function:: f(a)\n\n   :param a: First.\n\n      Second.\n   :returns:\n\n",
            ".. js:function:: $.getJSON(href)\n\n   :param string href:\n   :throws err:\n",
        ),
        &[
            concat!(
                "field-body\n            paragraph\n              strong ",
                "+generated @literal=\"1\"\n",
                "                text +generated \"a\"\n              text +generated \" – \"\n",
                "            paragraph\n              text \"First.\"\n",
                "            paragraph\n              text \"Second.\"\n",
            ),
            "text +generated \"Returns\"\n          field-body\n            paragraph\n",
            "text +generated \"Arguments\"\n          field-body\n            list list=bullet",
            concat!(
                "text +generated \"href\"\n                  text +generated \" (\"\n",
                "                  xref +generated link=unresolved name=\"string\" ",
                "@reftype=\"js:class\" @js:object=\"$\"\n",
                "                    code +generated \"string\"\n",
                "                  text +generated \")\"\n        field",
            ),
            "text +generated \"Throws\"",
            concat!(
                "xref +generated link=unresolved name=\"err\" @reftype=\"js:class\" ",
                "@js:object=\"$\"\n                code +generated \"err\"\n",
                "              text +generated \" – \"\n",
            ),
        ],
        None,
    );
}

#[test]
fn info_field_list_c_and_cpp() {
    expect_contains(
        concat!(
            ".. c:function:: int f(int x)\n\n   :param x: the x\n   :type x: int\n",
            "   :retval 0: ok\n   :rtype: int\n\n",
            ".. c:struct:: S\n\n   :param x: kept\n\n",
            ".. cpp:function:: template<typename T> void g(T t)\n\n   :tparam T: t\n",
            "   :param t: value\n   :throws std::bad_alloc: when\n   :retval 1: one\n",
        ),
        &[
            "text +generated \"Parameters\"\n          field-body\n            list list=bullet",
            concat!(
                "text +generated \"x\"\n                  text +generated \" (\"\n",
                "                  role +generated \"c-expr\"\n",
                "                    text +generated \"int\"\n",
                "                  text +generated \")\"",
            ),
            "text +generated \"Return values\"",
            "text +generated \"0\"",
            "text +generated \"Return type\"",
            "text \"int\"",
            "text \"Param x\"",
            "text +generated \"Template Parameters\"",
            "text +generated \"T\"",
            "text +generated \"Parameters\"\n          field-body\n            paragraph",
            "text +generated \"Throws\"",
            concat!(
                "role +generated \"cpp-expr\"\n",
                "                xref +generated link=unresolved name=\"std\" ",
                "@reftype=\"cpp:identifier\"",
            ),
            "text +generated \"Return values\"",
        ],
        None,
    );
}

// The group names come in the document's language from the host's labels.
#[test]
fn info_field_list_translated_labels() {
    let mut config = Config::default();
    config.labels.set(guidedoc::Label::Parameters, "Paramètres");
    config.labels.set(guidedoc::Label::Returns, "Renvoie");
    expect_contains(
        concat!(".. py:function:: f(a)\n\n   :param a: A.\n   :returns: B.\n   ", ":rtype: int\n",),
        &[
            "text +generated \"Paramètres\"",
            "text +generated \"Renvoie\"",
            "text +generated \"Return type\"",
        ],
        Some(&config),
    );
}

// myst-parser's Sphinx fixture fieldlist/index: fields in a Markdown object directive.
#[test]
fn info_field_list_markdown() {
    let mut myst = guidedoc_commonmark::Settings::default();
    myst.enable_extensions.insert(guidedoc_commonmark::SyntaxExtension::Fieldlist);
    md_expect_with(
        concat!(
            "```{py:function} send_message(sender, priority)\n\n",
            "Send a message to a recipient\n\n",
            ":param str sender: The person sending the message\n",
            ":param priority: The priority of the message, can be a number 1-5\n",
            ":type priority: int\n:return: the message id\n:rtype: int\n",
            ":raises ValueError: if the message_body exceeds 160 characters\n```\n",
        ),
        &[
            "field-list @class=\"myst\"",
            "text +generated \"Parameters\"",
            "text +generated \"sender\"",
            "name=\"str\"",
            "text +generated \" – \"",
            "text \"The person sending the message\"",
            "text +generated \"priority\"",
            "name=\"int\"",
            "text \"The priority of the message, can be a number 1-5\"",
            "text +generated \"Returns\"",
            "text \"the message id\"",
            "text +generated \"Return type\"",
            "name=\"int\"",
            "text +generated \"Raises\"",
            "name=\"ValueError\" @reftype=\"py:exc\"",
            "text \"if the message_body exceeds 160 characters\"",
        ],
        myst,
    );
}
