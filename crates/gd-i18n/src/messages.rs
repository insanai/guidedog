//! The diagnostics the translation pass reports.

use guidedoc::{Message, Severity, Status};

/// A translation whose markup the reader refuses; the original is kept.
/// Arguments: the translation, the reader's complaint.
pub const I18N_MARKUP: Message = Message {
    code: "i18n.translation.markup",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "TRANSLATION NOT USED",
    message: "The translation {0} of this message does not parse: {1}. The original text is \
              kept.",
    hint: "Fix the markup in the catalog's msgstr; it is read like the source document.",
};

/// A translation that drops references of the original (Sphinx's "inconsistent
/// references in translated message"); it is used anyway.
/// Arguments: the original, the translation.
pub const I18N_REFERENCES: Message = Message {
    code: "i18n.references.inconsistent",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "INCONSISTENT REFERENCES IN TRANSLATED MESSAGE",
    message: "The translation {1} does not have the same references as the original {0}.",
    hint: "Keep every reference, footnote, citation, and substitution of the original in the \
           msgstr, written the same way; only their text may change.",
};

/// A translation with a reference the original lacks, which cannot be resolved; the
/// original is kept. Arguments: the original, the translation.
pub const I18N_UNRESOLVED: Message = Message {
    code: "i18n.references.unknown",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "TRANSLATION NOT USED",
    message: "The translation {1} refers to something the original {0} does not, so it cannot \
              be resolved. The original text is kept.",
    hint: "Use the original's references in the msgstr; only their text may change.",
};
