use guidedoc::{
    AttributeKey as A, Builder, Fail, LinkInfo, ListInfo, ListStyle, NodeKind as K, Status,
};

use super::*;

fn field_open(b: &mut Builder<'_>, docfield: &str, name: &str) -> Result<(), Fail> {
    open(b, K::Field)?;
    if !docfield.is_empty() {
        custom(b, "docfield", docfield)?;
    }
    inline(b, K::FieldName, name)?;
    open(b, K::FieldBody).map(|_| ())
}

fn field_close(b: &mut Builder<'_>) -> Result<(), Fail> {
    shut(b)?;
    shut(b)
}

fn literal(b: &mut Builder<'_>, kind: K, s: &str) -> Result<(), Fail> {
    open(b, kind)?;
    custom(b, "literal", "1")?;
    text(b, s)?;
    shut(b)
}

fn field_xref(b: &mut Builder<'_>, reftype: &str, target: &str, display: K) -> Result<(), Fail> {
    let x = open(b, K::Xref)?;
    let name = b.add_text(target)?;
    b.set_link(x, LinkInfo { name, ..LinkInfo::UNRESOLVED })?;
    b.add_custom_attribute(x, "reftype", reftype)?;
    b.add_custom_attribute(x, "unstyled", "1")?;
    if display == K::Text {
        text(b, target)?;
    } else {
        literal(b, display, target)?;
    }
    shut(b)
}

fn bullets(b: &mut Builder<'_>) -> Result<(), Fail> {
    let list = open(b, K::List)?;
    b.set_list(list, ListInfo { style: ListStyle::Bullet, delimiter: b'*', start: 1 })
}

fn parameters(b: &mut Builder<'_>) -> Result<(), Fail> {
    field_open(b, "parameter", "Parameters")?;
    bullets(b)?;
    let params = [
        ("sender", "str", "The person sending the message"),
        ("priority", "int", "The priority of the message"),
    ];
    for (name, kind, description) in params {
        open(b, K::ListItem)?;
        open(b, K::Paragraph)?;
        literal(b, K::Strong, name)?;
        text(b, " (")?;
        field_xref(b, "py:class", kind, K::Emphasis)?;
        text(b, ")")?;
        text(b, " – ")?;
        text(b, description)?;
        shut(b)?;
        shut(b)?;
    }
    shut(b)?;
    field_close(b)
}

/// The tree the Sphinx reader builds for myst-parser's fixture fieldlist/index, whose
/// HTML Sphinx 9.1 writes as `info_fields_render_like_sphinx` expects.
pub fn sphinx_fields(b: &mut Builder<'_>) -> Result<(), Fail> {
    open(b, K::Desc)?;
    custom(b, "domain", "py")?;
    custom(b, "objtype", "function")?;
    open(b, K::DescSignature)?;
    attr(b, A::Id, "send_message")?;
    inline(b, K::DescName, "send_message")?;
    shut(b)?;
    open(b, K::DescContent)?;
    para(b, "Send a message to a recipient")?;
    open(b, K::FieldList)?;
    attr(b, A::Class, "myst")?;
    parameters(b)?;
    field_open(b, "returnvalue", "Returns")?;
    para(b, "the message id")?;
    field_close(b)?;
    field_open(b, "returntype", "Return type")?;
    open(b, K::Paragraph)?;
    field_xref(b, "py:class", "int", K::Text)?;
    shut(b)?;
    field_close(b)?;
    field_open(b, "exceptions", "Raises")?;
    open(b, K::Paragraph)?;
    field_xref(b, "py:exc", "ValueError", K::Strong)?;
    text(b, " – ")?;
    text(b, "if the message_body exceeds 160 characters")?;
    shut(b)?;
    field_close(b)?;
    field_open(b, "", "Note")?;
    field_close(b)?;
    shut(b)?;
    shut(b)?;
    shut(b)
}

#[test]
fn info_fields_render_like_sphinx() {
    let r = render(sphinx_fields, frag());
    assert_eq!(r.status, Status::Ok);
    assert_eq!(
        r.html,
        "<dl class=\"py function\">\n<dt id=\"send_message\" class=\"sig sig-object py\">\
         <span class=\"sig-name descname\">send_message</span></dt>\n<dd>\n\
         <p>Send a message to a recipient</p>\n<dl class=\"myst field-list simple\">\n\
         <dt class=\"field-odd\">Parameters<span class=\"colon\">:</span></dt>\n\
         <dd class=\"field-odd\"><ul class=\"simple\">\n\
         <li><p><strong>sender</strong> (<em>str</em>) – The person sending the message\
         </p></li>\n<li><p><strong>priority</strong> (<em>int</em>) – The priority of the \
         message</p></li>\n</ul>\n</dd>\n\
         <dt class=\"field-even\">Returns<span class=\"colon\">:</span></dt>\n\
         <dd class=\"field-even\"><p>the message id</p>\n</dd>\n\
         <dt class=\"field-odd\">Return type<span class=\"colon\">:</span></dt>\n\
         <dd class=\"field-odd\"><p>int</p>\n</dd>\n\
         <dt class=\"field-even\">Raises<span class=\"colon\">:</span></dt>\n\
         <dd class=\"field-even\"><p><strong>ValueError</strong> – if the message_body \
         exceeds 160 characters</p>\n</dd>\n\
         <dt class=\"field-odd\">Note<span class=\"colon\">:</span></dt>\n\
         <dd class=\"field-odd\"><p></p></dd>\n</dl>\n</dd>\n</dl>\n"
    );
}

// A field body with more than a paragraph, and a list item with more than one block,
// make a field list and a list no longer simple, as Docutils' SimpleListChecker says.
#[test]
fn field_lists_are_simple_as_docutils_says() {
    let r = render(
        |b| {
            open(b, K::Desc)?;
            open(b, K::DescContent)?;
            open(b, K::FieldList)?;
            field_open(b, "", "Example")?;
            para(b, "One.")?;
            leaf(b, K::CodeBlock, "x = 1\n")?;
            field_close(b)?;
            field_open(b, "", "Items")?;
            bullets(b)?;
            open(b, K::ListItem)?;
            para(b, "a")?;
            para(b, "b")?;
            shut(b)?;
            shut(b)?;
            field_close(b)?;
            shut(b)?;
            shut(b)?;
            shut(b)
        },
        frag(),
    );
    assert_eq!(r.status, Status::Ok);
    has(&r, "<dl class=\"field-list\">\n<dt class=\"field-odd\">Example");
    has(&r, "<dd class=\"field-odd\"><p>One.</p>\n<pre>");
    has(&r, "<dd class=\"field-even\"><ul>\n<li><p>a</p>\n<p>b</p>\n</li>\n</ul>\n</dd>\n");
}
