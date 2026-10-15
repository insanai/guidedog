//! The diagnostics of inline parsing and resolution. Every message has a hint.

use guidedoc::{Message, Severity, Status};

/// `rst.inline.unterminated`: unterminated inline markup.
pub(crate) const UNTERMINATED: Message = Message {
    code: "rst.inline.unterminated",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "UNTERMINATED INLINE MARKUP",
    message: "This {0} start-string {1} has no matching end-string, so I kept it as text.",
    hint: concat!(
        "Add the end-string after the marked-up text, or escape the start-string with a ",
        "backslash if it is meant literally.",
    ),
};

/// `rst.inline.multiple-roles`: two roles on one interpreted text.
pub(crate) const MULTIPLE_ROLES: Message = Message {
    code: "rst.inline.multiple-roles",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "TWO ROLES ON ONE INTERPRETED TEXT",
    message: "{0} has a role both before and after the text; only one is allowed.",
    hint: "Keep either the :role: prefix or the :role: suffix.",
};

/// `rst.inline.role-reference`: role on a hyperlink reference.
pub(crate) const ROLE_AND_REFERENCE: Message = Message {
    code: "rst.inline.role-reference",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "ROLE ON A HYPERLINK REFERENCE",
    message: "{0} has both an interpreted text role and a reference suffix.",
    hint: "Remove the role to make a hyperlink reference, or the trailing _ to apply the role.",
};

/// `rst.role.unknown`: unknown interpreted text role.
pub(crate) const ROLE_UNKNOWN: Message = Message {
    code: "rst.role.unknown",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "UNKNOWN INTERPRETED TEXT ROLE",
    message: "I do not know the role used in {0}.",
    hint: concat!(
        "Use a standard role such as :emphasis: or :code:, or define this one first with ",
        "the role directive, e.g. \".. role:: name\".",
    ),
};

/// `rst.role.raw`: raw role used directly.
pub(crate) const ROLE_RAW_DIRECT: Message = Message {
    code: "rst.role.raw",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "RAW ROLE USED DIRECTLY",
    message: "{0} uses the raw role, which has no output format of its own.",
    hint: concat!(
        "Define a role based on raw with a format, e.g. \".. role:: html(raw)\" with ",
        "\":format: html\", and use that role.",
    ),
};

/// `rst.role.pep`: invalid pep number.
pub(crate) const ROLE_PEP: Message = Message {
    code: "rst.role.pep",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INVALID PEP NUMBER",
    message: "{0} does not name a PEP; the number must be from 0 to 9999.",
    hint: "Write the PEP number only, e.g. :PEP:`8`.",
};

/// `rst.role.rfc`: invalid rfc number.
pub(crate) const ROLE_RFC: Message = Message {
    code: "rst.role.rfc",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INVALID RFC NUMBER",
    message: "{0} does not name an RFC; the number must be 1 or greater.",
    hint: "Write the RFC number, optionally with a section, e.g. :RFC:`2822#section-3`.",
};

/// `rst.reference.unresolved`: unknown target.
pub(crate) const REFERENCE_UNRESOLVED: Message = Message {
    code: "rst.reference.unresolved",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "UNKNOWN TARGET",
    message: "The reference {0} names no target in this document.",
    hint: concat!(
        "Add a target for it, e.g. \".. _name: https://example.org\", or fix the ",
        "spelling; names ignore case and repeated spaces.",
    ),
};

/// `rst.reference.ambiguous`: ambiguous target name.
pub(crate) const REFERENCE_AMBIGUOUS: Message = Message {
    code: "rst.reference.ambiguous",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "AMBIGUOUS TARGET NAME",
    message: "The reference {0} matches several targets or section titles.",
    hint: concat!(
        "Add an explicit target such as \".. _unique-name:\" before the section you mean, ",
        "and refer to that name.",
    ),
};

/// `rst.reference.anonymous`: anonymous hyperlink mismatch.
pub(crate) const ANONYMOUS_MISMATCH: Message = Message {
    code: "rst.reference.anonymous",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "ANONYMOUS HYPERLINK MISMATCH",
    message: "The document has {0} anonymous references but {1} anonymous targets.",
    hint: concat!(
        "Give every reference ending in __ exactly one \"__ target\" or \".. __: target\" ",
        "line, in the same order.",
    ),
};

/// `rst.target.duplicate`: duplicate target name.
pub(crate) const TARGET_DUPLICATE: Message = Message {
    code: "rst.target.duplicate",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "DUPLICATE TARGET NAME",
    message: concat!(
        "The target {0} repeats the name already defined by {1}, so neither can be ",
        "referred to by that name.",
    ),
    hint: "Rename one of the targets; names ignore case and repeated spaces.",
};

/// `rst.target.repeated`: target repeated.
pub(crate) const TARGET_REPEATED: Message = Message {
    code: "rst.target.repeated",
    severity: Severity::Note,
    status: Status::Ok,
    title: "TARGET REPEATED",
    message: concat!(
        "The target {0} repeats {1} and refers to the same place; the name keeps ",
        "referring to the first.",
    ),
    hint: "Remove the repeated target, or keep it: references are not ambiguous.",
};

/// `rst.target.ambiguous`: duplicate target name.
pub(crate) const TARGET_AMBIGUOUS: Message = Message {
    code: "rst.target.ambiguous",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "DUPLICATE TARGET NAME",
    message: concat!(
        "The target {0} repeats a name that several targets already define, so it cannot ",
        "be referred to by that name either.",
    ),
    hint: "Rename the targets; names ignore case and repeated spaces.",
};

/// `rst.target.implicit`: section name shadowed.
pub(crate) const TARGET_IMPLICIT: Message = Message {
    code: "rst.target.implicit",
    severity: Severity::Note,
    status: Status::Ok,
    title: "SECTION NAME SHADOWED",
    message: "The name of {0} is also defined by {1}.",
    hint: concat!(
        "Rename the explicit target or the section title to link to the section by name; ",
        "the explicit target wins, and titles that repeat cannot be linked by name.",
    ),
};

/// `rst.target.cycle`: indirect target cycle.
pub(crate) const TARGET_CYCLE: Message = Message {
    code: "rst.target.cycle",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INDIRECT TARGET CYCLE",
    message: "The indirect target {0} leads back to itself.",
    hint: "Point one of the targets in the chain at a URI or at an internal target.",
};

/// `rst.target.unresolved`: indirect target not found.
pub(crate) const TARGET_UNRESOLVED: Message = Message {
    code: "rst.target.unresolved",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INDIRECT TARGET NOT FOUND",
    message: "The indirect target {0} refers to a name no target defines.",
    hint: "Define the referenced target, or point this one at a URI.",
};

/// `rst.footnote.references`: too many footnote references.
pub(crate) const FOOTNOTE_TOO_MANY: Message = Message {
    code: "rst.footnote.references",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "TOO MANY FOOTNOTE REFERENCES",
    message: "The reference {0} has no corresponding {1} footnote left.",
    hint: "Add one footnote per [#]_ or [*]_ reference, in the same order.",
};

/// `rst.substitution.undefined`: undefined substitution.
pub(crate) const SUBSTITUTION_UNDEFINED: Message = Message {
    code: "rst.substitution.undefined",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "UNDEFINED SUBSTITUTION",
    message: "The substitution {0} is not defined.",
    hint: "Add a definition, e.g. \".. |name| replace:: text\".",
};

/// `rst.substitution.duplicate`: duplicate substitution definition.
pub(crate) const SUBSTITUTION_DUPLICATE: Message = Message {
    code: "rst.substitution.duplicate",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "DUPLICATE SUBSTITUTION DEFINITION",
    message: "The substitution definition {0} repeats the name defined by {1}.",
    hint: "Rename or remove one of the definitions.",
};

/// `rst.substitution.cycle`: circular substitution.
pub(crate) const SUBSTITUTION_CYCLE: Message = Message {
    code: "rst.substitution.cycle",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "CIRCULAR SUBSTITUTION",
    message: "The substitution {0} is used inside its own definition.",
    hint: "Break the cycle so no definition refers to itself, directly or indirectly.",
};

/// `rst.substitution.depth`: substitutions nested too deeply.
pub(crate) const SUBSTITUTION_DEPTH: Message = Message {
    code: "rst.substitution.depth",
    severity: Severity::Error,
    status: Status::Limit,
    title: "SUBSTITUTIONS NESTED TOO DEEPLY",
    message: "The substitution {0} nests more than {1} definitions deep.",
    hint: "Flatten the substitution definitions, or raise limits.max_expansion.",
};

/// `rst.docinfo.paragraph`: bibliographic field is not one paragraph.
pub(crate) const DOCINFO_PARAGRAPH: Message = Message {
    code: "rst.docinfo.paragraph",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "BIBLIOGRAPHIC FIELD IS NOT ONE PARAGRAPH",
    message: concat!(
        "The bibliographic field {0} must hold a single paragraph, so I kept it as an ",
        "ordinary field.",
    ),
    hint: "Write the field's value as one paragraph.",
};

/// `rst.docinfo.empty`: empty bibliographic field.
pub(crate) const DOCINFO_EMPTY: Message = Message {
    code: "rst.docinfo.empty",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "EMPTY BIBLIOGRAPHIC FIELD",
    message: "The bibliographic field {0} is empty, so I kept it as an ordinary field.",
    hint: "Give the field a value, or remove it.",
};

/// `rst.docinfo.repeated`: repeated bibliographic field.
pub(crate) const DOCINFO_REPEATED: Message = Message {
    code: "rst.docinfo.repeated",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "REPEATED BIBLIOGRAPHIC FIELD",
    message: "A document has only one {0} field; I kept this one as an ordinary field.",
    hint: "Merge the fields into one.",
};

/// `rst.docinfo.authors`: authors field cannot be split.
pub(crate) const DOCINFO_AUTHORS: Message = Message {
    code: "rst.docinfo.authors",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "AUTHORS FIELD CANNOT BE SPLIT",
    message: concat!(
        "The field {0} must hold one paragraph of names separated by \";\" or \",\", one ",
        "paragraph per author, or a bullet list with one paragraph per item.",
    ),
    hint: "Rewrite the list of authors in one of these forms.",
};

/// Every message of the crate, for documentation and tests.
pub const MESSAGES: &[Message] = &[
    UNTERMINATED,
    MULTIPLE_ROLES,
    ROLE_AND_REFERENCE,
    ROLE_UNKNOWN,
    ROLE_RAW_DIRECT,
    ROLE_PEP,
    ROLE_RFC,
    REFERENCE_UNRESOLVED,
    REFERENCE_AMBIGUOUS,
    ANONYMOUS_MISMATCH,
    TARGET_DUPLICATE,
    TARGET_REPEATED,
    TARGET_AMBIGUOUS,
    TARGET_IMPLICIT,
    TARGET_CYCLE,
    TARGET_UNRESOLVED,
    FOOTNOTE_TOO_MANY,
    SUBSTITUTION_UNDEFINED,
    SUBSTITUTION_DUPLICATE,
    SUBSTITUTION_CYCLE,
    SUBSTITUTION_DEPTH,
    DOCINFO_PARAGRAPH,
    DOCINFO_EMPTY,
    DOCINFO_REPEATED,
    DOCINFO_AUTHORS,
];
