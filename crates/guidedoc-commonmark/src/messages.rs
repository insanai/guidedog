//! The reader's messages. Every message has a hint.

use guidedoc::{Message, Severity, Status};

pub(crate) const NESTING: Message = Message {
    code: "commonmark.nesting",
    severity: Severity::Error,
    status: Status::Limit,
    title: "BLOCKS NESTED TOO DEEPLY",
    message: "Block quotes and list items are nested more than {0} levels deep.",
    hint: "Flatten the nesting of quotes and lists, or raise limits.max_depth.",
};

pub(crate) const SLUG_FUNC: Message = Message {
    code: "myst.slug-func",
    severity: Severity::Warning,
    status: Status::Unsupported,
    title: "HEADING SLUG FUNCTION NOT SUPPORTED",
    message: "myst_heading_slug_func names the Python function {0}, which cannot run here; \
        heading anchors use GitHub-style slugs instead.",
    hint: "Remove myst_heading_slug_func, or accept the default slugs.",
};

pub(crate) const HEADING_SKIP: Message = Message {
    code: "myst.header",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "HEADING LEVEL SKIPPED",
    message: "Non-consecutive header level increase; {0} to {1}.",
    hint: "Use the next heading level, so the sections nest as written.",
};

pub(crate) const HEADING_NESTED: Message = Message {
    code: "myst.header",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "HEADING INSIDE ANOTHER ELEMENT",
    message: "A heading nested in this element can lead to unexpected outcomes; it becomes a \
        rubric.",
    hint: "Move the heading out of the block quote, list, or directive.",
};

pub(crate) const DIRECTIVE_UNKNOWN: Message = Message {
    code: "myst.directive.unknown",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "UNKNOWN DIRECTIVE",
    message: "Unknown directive type {0}.",
    hint: "Check the name, or enable the Sphinx extension that provides it.",
};

pub(crate) const DIRECTIVE_OPTIONS: Message = Message {
    code: "myst.directive.options",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "DIRECTIVE OPTIONS ARE NOT VALID",
    message: "The options of directive {0} could not be read: {1}.",
    hint: "Write options as `:name: value` lines, or as a YAML mapping between `---` lines.",
};

pub(crate) const DIRECTIVE_OPTION_VALUE: Message = Message {
    code: "myst.directive.option-value",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "DIRECTIVE OPTION VALUE IS NOT A STRING",
    message: "Option {1} of directive {0} is not a string.",
    hint: "Enclose the value in quotes, e.g. `:class: \"[1]\"`.",
};

pub(crate) const DIRECTIVE_NO_HANDLER: Message = Message {
    code: "myst.directive.unsupported",
    severity: Severity::Warning,
    status: Status::Unsupported,
    title: "DIRECTIVES NEED THE SPHINX LAYER",
    message: "Directive {0} was not run: this conversion reads Markdown without directive \
        handlers.",
    hint: "Build the project with Guidedog, which reads Markdown with Sphinx's directives.",
};

pub(crate) const ROLE_UNKNOWN: Message = Message {
    code: "myst.role.unknown",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "UNKNOWN ROLE",
    message: "Unknown interpreted text role {0}.",
    hint: "Check the role name, or enable the Sphinx extension that provides it.",
};

pub(crate) const FRONT_MATTER: Message = Message {
    code: "myst.topmatter",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "FRONT MATTER IS NOT VALID YAML",
    message: "Front matter block: {0}.",
    hint: "Fix the YAML between the `---` lines; values with `:` or `#` need quotes.",
};

pub(crate) const FRONT_MATTER_TYPE: Message = Message {
    code: "myst.topmatter",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "FRONT MATTER IS NOT A MAPPING",
    message: "Front matter must be a YAML mapping of keys to values.",
    hint: "Write the front matter as `key: value` lines.",
};

pub(crate) const HTML_META: Message = Message {
    code: "myst.html-meta",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "HTML META TAG IS NOT VALID",
    message: "Error parsing the meta tag {0}: {1}.",
    hint: "Write keys as `name`, or `attribute=value` pairs such as `http-equiv=Content-Type`.",
};

pub(crate) const FOOTNOTE_DUPLICATE: Message = Message {
    code: "myst.footnote.duplicate",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "FOOTNOTE DEFINED TWICE",
    message: "Multiple footnote definitions found for label {0}; the first one is used.",
    hint: "Give each footnote definition its own label.",
};

pub(crate) const SUBSTITUTION_UNKNOWN: Message = Message {
    code: "myst.substitution",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "SUBSTITUTION NOT DEFINED",
    message: "Substitution error: {0} is undefined.",
    hint: "Define it in the front matter's substitutions, or in myst_substitutions.",
};

pub(crate) const SUBSTITUTION_CIRCULAR: Message = Message {
    code: "myst.substitution",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "CIRCULAR SUBSTITUTION",
    message: "Circular substitution reference: {0}.",
    hint: "Break the cycle so no substitution refers to itself, directly or through others.",
};

pub(crate) const SUBSTITUTION_EXPRESSION: Message = Message {
    code: "myst.substitution",
    severity: Severity::Warning,
    status: Status::Unsupported,
    title: "SUBSTITUTION EXPRESSION NOT SUPPORTED",
    message: "The Jinja expression {0} cannot be evaluated: {1}.",
    hint: "Use names, dotted names, quoted strings, `+` or `~`, and the filters upper, lower, \
        title, capitalize, trim, replace, and default.",
};

pub(crate) const SYNTAX_RULE_UNKNOWN: Message = Message {
    code: "myst.disable-syntax",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "UNKNOWN SYNTAX RULE",
    message: "myst_disable_syntax names {0}, which is not a syntax rule.",
    hint: "Use markdown-it rule names, such as emphasis, table, or html_block.",
};

pub(crate) const REFERENCE_UNRESOLVED: Message = Message {
    code: "myst.reference.unresolved",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "REFERENCE HAS NO TARGET",
    message: "Unknown target name: {0}.",
    hint: "Define the footnote or target this refers to, or correct its name.",
};

pub(crate) const FIGURE_MD: Message = Message {
    code: "myst.figure-md",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "MARKDOWN FIGURE IS NOT AN IMAGE AND A CAPTION",
    message: "The content of figure-md {0} must be an image and one caption paragraph.",
    hint: "Write the image (Markdown or <img>), a blank line, then the caption.",
};

pub(crate) const DUPLICATE_REFERENCE: Message = Message {
    code: "myst.duplicate-def",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "LINK REFERENCE DEFINED TWICE",
    message: "Duplicate reference definition: {0}; the first definition is used.",
    hint: "Give each link reference definition its own label.",
};
