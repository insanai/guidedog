//! The reader's messages. They follow Docutils' severity intent: problems Docutils
//! reports at level 3 or above are errors, level 2 problems are warnings. `{0}` and `{1}`
//! name the arguments, usually quoted source spans.

use guidedoc::{Message, Severity, Status};

/// Inconsistent section level.
pub const SECTION_LEVEL: Message = Message {
    code: "rst.section.level",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INCONSISTENT SECTION LEVEL",
    message: "The title {0} uses an adornment that does not fit the section hierarchy.",
    hint: concat!(
        "Adornment styles take levels in the order they first appear; reuse the style of ",
        "a parent or sibling title, or introduce a new style only one level deeper.",
    ),
};

/// Title underline too short.
pub const SECTION_UNDERLINE_SHORT: Message = Message {
    code: "rst.section.underline-short",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "TITLE UNDERLINE TOO SHORT",
    message: "The underline of {0} is shorter than the title text.",
    hint: "Extend the underline so it is at least as long as the title.",
};

/// Title overline too short.
pub const SECTION_OVERLINE_SHORT: Message = Message {
    code: "rst.section.overline-short",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "TITLE OVERLINE TOO SHORT",
    message: "The overline and underline of {0} are shorter than the title text.",
    hint: "Extend both adornment lines so they cover the whole title.",
};

/// Section title in a body element.
pub const SECTION_UNEXPECTED: Message = Message {
    code: "rst.section.unexpected",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "SECTION TITLE IN A BODY ELEMENT",
    message: concat!(
        "{0} looks like a section title or transition, but it is nested in a body ",
        "element.",
    ),
    hint: concat!(
        "Sections and transitions may only appear at the top level; move the title out of ",
        "the list, quote, table, or directive, or change the adornment line.",
    ),
};

/// Incomplete section title.
pub const SECTION_INCOMPLETE: Message = Message {
    code: "rst.section.incomplete",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INCOMPLETE SECTION TITLE",
    message: "The overlined title {0} has no underline before the end of the input.",
    hint: "Add an underline that repeats the overline exactly.",
};

/// Missing title underline.
pub const SECTION_NO_UNDERLINE: Message = Message {
    code: "rst.section.underline-missing",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "MISSING TITLE UNDERLINE",
    message: "The overlined title {0} is not followed by a matching underline.",
    hint: "Put an underline that repeats the overline right below the title line.",
};

/// Title overline and underline differ.
pub const SECTION_MISMATCH: Message = Message {
    code: "rst.section.mismatch",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "TITLE OVERLINE AND UNDERLINE DIFFER",
    message: "The overline and underline of {0} are not the same.",
    hint: "Use the same character and length for both adornment lines.",
};

/// Invalid title or transition.
pub const SECTION_INVALID: Message = Message {
    code: "rst.section.invalid",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INVALID TITLE OR TRANSITION",
    message: "{0} is neither a section title nor a transition.",
    hint: concat!(
        "Put blank lines around a transition, or a text line between the adornment lines ",
        "of an overlined title.",
    ),
};

/// Transition at the start.
pub const TRANSITION_START: Message = Message {
    code: "rst.transition.start",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "TRANSITION AT THE START",
    message: "The transition {0} begins a document or section.",
    hint: "Put body content before the transition, or remove it.",
};

/// Adjacent transitions.
pub const TRANSITION_ADJACENT: Message = Message {
    code: "rst.transition.adjacent",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "ADJACENT TRANSITIONS",
    message: "The transition {0} directly follows another transition.",
    hint: "Separate transitions with at least one body element, or remove one.",
};

/// Transition at the end.
pub const TRANSITION_END: Message = Message {
    code: "rst.transition.end",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "TRANSITION AT THE END",
    message: "The document ends with the transition {0}.",
    hint: "Remove the final transition or add content after it.",
};

/// Unexpected indentation.
pub const INDENT_UNEXPECTED: Message = Message {
    code: "rst.indent.unexpected",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "UNEXPECTED INDENTATION",
    message: concat!(
        "The line {0} is indented, but the text before it is not a construct that takes ",
        "indented content.",
    ),
    hint: concat!(
        "Insert a blank line before the indented text to make it a block quote, or align ",
        "it with the paragraph.",
    ),
};

/// Missing blank line.
pub const BLOCK_UNINDENT: Message = Message {
    code: "rst.block.unindent",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "MISSING BLANK LINE",
    message: "A {1} ends without a blank line before {0}.",
    hint: "Insert a blank line after the construct, or indent the line to continue it.",
};

/// Literal block expected.
pub const LITERAL_MISSING: Message = Message {
    code: "rst.literal.missing",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "LITERAL BLOCK EXPECTED",
    message: "The paragraph {0} ends with \"::\" but no literal block follows.",
    hint: "Indent the literal text below the paragraph, or remove the \"::\".",
};

/// Inconsistent literal quoting.
pub const LITERAL_QUOTING: Message = Message {
    code: "rst.literal.quoting",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INCONSISTENT LITERAL QUOTING",
    message: "The quoted literal line {0} does not start with the same quote character.",
    hint: "Start every line of a quoted literal block with the same character.",
};

/// Malformed table.
pub const TABLE_MALFORMED: Message = Message {
    code: "rst.table.malformed",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "MALFORMED TABLE",
    message: "The table at {0} is malformed: {1}.",
    hint: concat!(
        "Check that borders line up and every row has the same width; tables are drawn in ",
        "characters, so tabs and misaligned edges break them.",
    ),
};

/// Table data does not fit.
pub const TABLE_SHAPE: Message = Message {
    code: "rst.table.shape",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "TABLE DATA DOES NOT FIT",
    message: "The table directive {0} has a problem: {1}.",
    hint: "Make header rows, stub columns, and widths agree with the table's rows and columns.",
};

/// Unknown directive option.
pub const DIRECTIVE_OPTION_UNKNOWN: Message = Message {
    code: "rst.directive.option.unknown",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "UNKNOWN DIRECTIVE OPTION",
    message: "The option {0} is not an option of the \"{1}\" directive.",
    hint: concat!(
        "Remove the option or fix its name; options must be spelled as the directive ",
        "documents them.",
    ),
};

/// Duplicate directive option.
pub const DIRECTIVE_OPTION_DUPLICATE: Message = Message {
    code: "rst.directive.option.duplicate",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "DUPLICATE DIRECTIVE OPTION",
    message: "The option {0} is given more than once.",
    hint: "Keep one occurrence of each option.",
};

/// Too many directive options.
pub const DIRECTIVE_OPTIONS_LIMIT: Message = Message {
    code: "rst.directive.option.limit",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "TOO MANY DIRECTIVE OPTIONS",
    message: "A directive takes at most {0} options; this one is over the limit.",
    hint: "Remove options the directive does not need.",
};

/// Invalid option value.
pub const DIRECTIVE_OPTION_INVALID: Message = Message {
    code: "rst.directive.option.invalid",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INVALID OPTION VALUE",
    message: "The option {0} has an invalid value: {1}.",
    hint: "Correct the value to the form the option expects.",
};

/// Invalid option block.
pub const DIRECTIVE_OPTION_BLOCK: Message = Message {
    code: "rst.directive.option.block",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INVALID OPTION BLOCK",
    message: "The line {0} in the options of a directive is not an option.",
    hint: concat!(
        "Write options as \":name: value\" lines, and separate content from them with a ",
        "blank line.",
    ),
};

/// Missing directive argument.
pub const DIRECTIVE_ARGUMENT_MISSING: Message = Message {
    code: "rst.directive.argument.missing",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "MISSING DIRECTIVE ARGUMENT",
    message: "The directive {0} needs an argument after \"::\".",
    hint: "Add the argument on the directive line, for example the title or URI.",
};

/// Too many directive arguments.
pub const DIRECTIVE_ARGUMENT_EXTRA: Message = Message {
    code: "rst.directive.argument.extra",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "TOO MANY DIRECTIVE ARGUMENTS",
    message: "The directive {0} takes at most one word as its argument.",
    hint: concat!(
        "Remove the extra words, or separate content from the directive line with a blank ",
        "line.",
    ),
};

/// Directive content missing.
pub const DIRECTIVE_CONTENT_MISSING: Message = Message {
    code: "rst.directive.content.missing",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "DIRECTIVE CONTENT MISSING",
    message: "The directive {0} needs indented content.",
    hint: "Indent the content below the directive, separated by a blank line.",
};

/// Directive takes no content.
pub const DIRECTIVE_CONTENT_FORBIDDEN: Message = Message {
    code: "rst.directive.content.forbidden",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "DIRECTIVE TAKES NO CONTENT",
    message: "The directive {0} does not accept content.",
    hint: "Remove the indented text, or dedent it to end the directive.",
};

/// Directive not allowed here.
pub const DIRECTIVE_CONTEXT: Message = Message {
    code: "rst.directive.context",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "DIRECTIVE NOT ALLOWED HERE",
    message: "The directive {0} cannot be used in this context: {1}.",
    hint: "Move the directive to a place where it is allowed.",
};

/// Remote resources are refused.
pub const DIRECTIVE_URL: Message = Message {
    code: "rst.directive.url",
    severity: Severity::Error,
    status: Status::Policy,
    title: "REMOTE RESOURCES ARE REFUSED",
    message: "The option {0} asks to fetch a URL, which the reader never does.",
    hint: "Download the resource and use :file: or inline content instead.",
};

/// Unsupported encoding.
pub const DIRECTIVE_ENCODING: Message = Message {
    code: "rst.directive.encoding",
    severity: Severity::Error,
    status: Status::Unsupported,
    title: "UNSUPPORTED ENCODING",
    message: "The option {0} names an encoding other than UTF-8.",
    hint: "Convert the resource to UTF-8 and drop the option, or write utf-8.",
};

/// Include parser not supported.
pub const DIRECTIVE_PARSER: Message = Message {
    code: "rst.directive.parser",
    severity: Severity::Error,
    status: Status::Unsupported,
    title: "INCLUDE PARSER NOT SUPPORTED",
    message: "The option {0} selects another parser for the included file.",
    hint: "Convert the file to reStructuredText, or include it with :literal:.",
};

/// Both file and content.
pub const DIRECTIVE_FILE_AND_CONTENT: Message = Message {
    code: "rst.directive.file-and-content",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "BOTH FILE AND CONTENT",
    message: "The directive {0} has content and also names a file.",
    hint: "Use either inline content or the :file: option, not both.",
};

/// Include marker not found.
pub const INCLUDE_TEXT_NOT_FOUND: Message = Message {
    code: "rst.include.text-not-found",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INCLUDE MARKER NOT FOUND",
    message: "The text of option {0} does not occur in the included file.",
    hint: "Check the marker text; it must appear verbatim in the included file.",
};

/// Figure caption is not a paragraph.
pub const FIGURE_CAPTION: Message = Message {
    code: "rst.figure.caption",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "FIGURE CAPTION IS NOT A PARAGRAPH",
    message: "The figure content {0} must start with a paragraph or an empty comment.",
    hint: concat!(
        "Make the caption a plain paragraph, or start with \"..\" to give a legend ",
        "without a caption.",
    ),
};

/// Malformed csv data.
pub const CSV_ERROR: Message = Message {
    code: "rst.csv.malformed",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "MALFORMED CSV DATA",
    message: "The CSV data at {0} is malformed: {1}.",
    hint: "Close every quoted field and escape quote characters by doubling them.",
};

/// Malformed list table.
pub const LIST_TABLE_SHAPE: Message = Message {
    code: "rst.list-table.shape",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "MALFORMED LIST TABLE",
    message: "The list table at {0} is malformed: {1}.",
    hint: concat!(
        "Write a bullet list of rows, each row holding a bullet list with the same number ",
        "of cells.",
    ),
};

/// Unknown role.
pub const ROLE_UNKNOWN: Message = Message {
    code: "rst.role.unknown",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "UNKNOWN ROLE",
    message: "The role {0} is neither a standard role nor defined earlier.",
    hint: "Define it first with the role directive, or use a standard role name.",
};

/// Invalid role definition.
pub const ROLE_INVALID: Message = Message {
    code: "rst.role.invalid",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INVALID ROLE DEFINITION",
    message: "The role definition {0} is not of the form name or name(base).",
    hint: "Write the new role name, optionally followed by the base role in parentheses.",
};

/// Too many roles.
pub const ROLE_CAPACITY: Message = Message {
    code: "rst.role.capacity",
    severity: Severity::Error,
    status: Status::Limit,
    title: "TOO MANY ROLES",
    message: "The role {0} does not fit; the document defines more than {1} roles.",
    hint: "Define fewer custom roles; included files count toward the same limit.",
};

/// Class applies to nothing.
pub const CLASS_DANGLING: Message = Message {
    code: "rst.class.dangling",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "CLASS APPLIES TO NOTHING",
    message: "No element follows the class directive {0}.",
    hint: concat!(
        "Put the element the class is meant for after the directive, or indent it as the ",
        "directive's content.",
    ),
};

/// Invalid substitution definition.
pub const SUBSTITUTION_INVALID: Message = Message {
    code: "rst.substitution.invalid",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INVALID SUBSTITUTION DEFINITION",
    message: "The substitution definition {0} is empty or does not use a directive.",
    hint: "Write \".. |name| replace:: text\", or use unicode, date, image, or raw.",
};

/// Directive not allowed in a substitution.
pub const SUBSTITUTION_DIRECTIVE: Message = Message {
    code: "rst.substitution.directive",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "DIRECTIVE NOT ALLOWED IN A SUBSTITUTION",
    message: "The directive {0} does not produce inline content for a substitution.",
    hint: "Use replace, unicode, date, image, or raw in substitution definitions.",
};

/// Replace text is not one paragraph.
pub const SUBSTITUTION_REPLACE: Message = Message {
    code: "rst.substitution.replace",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "REPLACE TEXT IS NOT ONE PARAGRAPH",
    message: "The replacement text {0} must be a single paragraph.",
    hint: "Remove blank lines from the replacement text.",
};

/// Malformed hyperlink target.
pub const TARGET_MALFORMED: Message = Message {
    code: "rst.target.malformed",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "MALFORMED HYPERLINK TARGET",
    message: "The hyperlink target {0} has no valid name.",
    hint: concat!(
        "Write \".. _name: URI\"; quote names that contain colons with backquotes or ",
        "escape the colons.",
    ),
};

/// No date available.
pub const DATE_NO_TODAY: Message = Message {
    code: "rst.date.today",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "NO DATE AVAILABLE",
    message: "The date directive {0} needs today's date, but the host supplied none.",
    hint: "Set read options today to a date such as 2026-09-29.",
};

/// Invalid character code.
pub const UNICODE_INVALID: Message = Message {
    code: "rst.unicode.invalid",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INVALID CHARACTER CODE",
    message: "The character code {0} is not a Unicode scalar value.",
    hint: "Use a code between 0 and 10FFFF that is not a surrogate.",
};

/// Invalid meta data.
pub const META_INVALID: Message = Message {
    code: "rst.meta.invalid",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INVALID META DATA",
    message: "The meta directive line {0} is not a field with a value.",
    hint: "Write meta data as a field list, for example \":keywords: rst, odin\".",
};

/// Unknown directive.
pub const DIRECTIVE_UNKNOWN: Message = Message {
    code: "rst.directive.unknown",
    severity: Severity::Error,
    status: Status::Unsupported,
    title: "UNKNOWN DIRECTIVE",
    message: "I do not know the directive {0}.",
    hint: concat!(
        "Check the spelling against the standard reStructuredText directives. Sphinx ",
        "directives such as toctree or autofunction belong to an extension layer.",
    ),
};

/// An inline span outside the source being read.
pub const INLINE_SPAN: Message = Message {
    code: "rst.inline.span",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INLINE SPAN OUTSIDE THE SOURCE",
    message: "The span {0} asked for inline parsing is not part of the source being read.",
    hint: "Pass a span of ctx.source that lies within its text.",
};
