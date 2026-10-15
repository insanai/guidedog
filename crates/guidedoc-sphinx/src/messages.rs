//! Messages of the Sphinx layer. Sphinx's own warnings stay warnings; problems Sphinx
//! reports as errors (a directive that produces nothing) are errors here too.

use guidedoc::{Message, Severity, Status};

/// Malformed index entry.
pub(crate) const INDEX_ENTRY: Message = Message {
    code: "sphinx.index.entry",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "MALFORMED INDEX ENTRY",
    message: "The index entry {0} does not have the parts its type needs.",
    hint: concat!(
        "Separate the parts with \";\": a pair and a see/seealso entry need two, a ",
        "triple needs three, for example \"pair: loop; statement\".",
    ),
};

/// Hlist needs one bullet list.
pub(crate) const HLIST_CONTENT: Message = Message {
    code: "sphinx.hlist.content",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "HLIST NEEDS ONE BULLET LIST",
    message: "The content of {0} must be a single bullet list.",
    hint: "Write the items as one bullet list, each item starting with \"* \".",
};

/// Malformed glossary.
pub(crate) const GLOSSARY_CONTENT: Message = Message {
    code: "sphinx.glossary.content",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "MALFORMED GLOSSARY",
    message: "The glossary line {0} is neither a term nor an indented definition.",
    hint: concat!(
        "Write each term flush left and its definition indented below it; a term ",
        "must be followed by a definition.",
    ),
};

/// Duplicate glossary term.
pub(crate) const GLOSSARY_DUPLICATE: Message = Message {
    code: "sphinx.glossary.duplicate",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "DUPLICATE GLOSSARY TERM",
    message: "The term {0} is already defined in this document.",
    hint: "Define each term once; refer to it with :term: elsewhere.",
};

/// Invalid line specification.
pub(crate) const LINES_SPEC: Message = Message {
    code: "sphinx.code.lines",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "INVALID LINE SPECIFICATION",
    message: "The line specification {0} is not valid for this code.",
    hint: concat!(
        "Use comma-separated line numbers and ranges, such as 1,3-5 or 7-, within ",
        "the number of lines.",
    ),
};

/// Dedent removes text.
pub(crate) const DEDENT_TEXT: Message = Message {
    code: "sphinx.code.dedent",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "DEDENT REMOVES TEXT",
    message: "The :dedent: of {0} removes characters that are not white space.",
    hint: "Lower the :dedent: value, or omit the value to remove the common indentation.",
};

/// Conflicting literalinclude options.
pub(crate) const INCLUDE_OPTIONS: Message = Message {
    code: "sphinx.literalinclude.options",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "CONFLICTING LITERALINCLUDE OPTIONS",
    message: "The options of {0} cannot be combined: {1}.",
    hint: "Keep one option of each conflicting pair.",
};

/// Literalinclude selection found nothing.
pub(crate) const INCLUDE_PATTERN: Message = Message {
    code: "sphinx.literalinclude.pattern",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "LITERALINCLUDE SELECTION FOUND NOTHING",
    message: "The selection {0} matches nothing in the included file.",
    hint: "Check the text or object name against the file's contents.",
};

/// Only utf-8 is supported.
pub(crate) const ENCODING: Message = Message {
    code: "sphinx.encoding",
    severity: Severity::Error,
    status: Status::Unsupported,
    title: "ONLY UTF-8 IS SUPPORTED",
    message: "The encoding {0} is not UTF-8.",
    hint: "Convert the file to UTF-8, for example with iconv, and drop :encoding:.",
};

/// Unknown role.
pub(crate) const ROLE_UNKNOWN: Message = Message {
    code: "sphinx.role.unknown",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "UNKNOWN ROLE",
    message: "The role {0} is not defined.",
    hint: "Use a standard, Sphinx, or domain role, or define it with the role directive.",
};

/// Signature not understood.
pub(crate) const SIGNATURE: Message = Message {
    code: "sphinx.signature",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "SIGNATURE NOT UNDERSTOOD",
    message: "The signature {0} could not be parsed; it is shown as written.",
    hint: "Check the signature's syntax for this domain.",
};

/// Malformed option description.
pub(crate) const OPTION_DESCRIPTION: Message = Message {
    code: "sphinx.option.malformed",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "MALFORMED OPTION DESCRIPTION",
    message: concat!(
        "The option description {0} should look like \"opt\", \"-opt args\", \"--opt ",
        "args\", \"/opt args\", or \"+opt args\".",
    ),
    hint: "Start each option with -, --, /, or +, separating options with \", \".",
};

/// Objects nested too deeply.
pub(crate) const NESTING: Message = Message {
    code: "sphinx.nesting",
    severity: Severity::Error,
    status: Status::Limit,
    title: "OBJECTS NESTED TOO DEEPLY",
    message: "Object descriptions are nested more than 32 levels deep at {0}.",
    hint: "Flatten the nesting of classes and namespaces.",
};

/// Invalid cwe number.
pub(crate) const CWE_INVALID: Message = Message {
    code: "sphinx.role.cwe",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INVALID CWE NUMBER",
    message: "{0} does not name a CWE by number.",
    hint: "Write a number, optionally with an anchor: :cwe:`79`.",
};

/// Invalid pep number.
pub(crate) const PEP_INVALID: Message = Message {
    code: "sphinx.role.pep",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INVALID PEP NUMBER",
    message: "{0} does not name a PEP by number.",
    hint: "Write a number, optionally with an anchor: :pep:`8#introduction`.",
};

/// Invalid rfc number.
pub(crate) const RFC_INVALID: Message = Message {
    code: "sphinx.role.rfc",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INVALID RFC NUMBER",
    message: "{0} does not name an RFC by number.",
    hint: "Write a number, optionally with an anchor: :rfc:`2822#section-3`.",
};

/// Graphviz needs one source.
pub(crate) const GRAPHVIZ_SOURCE: Message = Message {
    code: "sphinx.graphviz.source",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "GRAPHVIZ NEEDS ONE SOURCE",
    message: "The directive {0} cannot be drawn: {1}.",
    hint: "Give the DOT source either as the directive's content or as a file argument, not both.",
};

/// Invalid external reference.
pub(crate) const EXTERNAL_SUFFIX: Message = Message {
    code: "sphinx.external.suffix",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "INVALID EXTERNAL REFERENCE",
    message: "The role in {0} is not :external:role: or :external:domain:role:.",
    hint: "Write, for example, :external:py:func: or :external+python:ref:.",
};

/// Unknown domain.
pub(crate) const EXTERNAL_DOMAIN: Message = Message {
    code: "sphinx.external.domain",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "UNKNOWN DOMAIN",
    message: "The domain of the external cross-reference {0} is not known.",
    hint: "Use one of the domains std, py, c, cpp, js, rst, or math.",
};

/// Unknown external role.
pub(crate) const EXTERNAL_ROLE: Message = Message {
    code: "sphinx.external.role",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "UNKNOWN EXTERNAL ROLE",
    message: "The role of the external cross-reference {0} is not a role of its domain.",
    hint: "Name a cross-reference role, such as :external:py:class: or :external:ref:.",
};

/// Autodoc nested too deeply.
pub(crate) const AUTODOC_NESTED: Message = Message {
    code: "sphinx.autodoc.nesting",
    severity: Severity::Error,
    status: Status::Limit,
    title: "AUTODOC NESTED TOO DEEPLY",
    message: "Autodoc directives generate autodoc directives more than {0} levels deep.",
    hint: "Look for a docstring that documents itself with automodule or autoclass.",
};
