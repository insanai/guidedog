//! The renderer's diagnostics. Every one has a title, a message, and a hint; codes are
//! stable, so hosts may match on them.

use guidedoc::{Message, Severity, Status};

/// Math the adapter cannot translate, under the refuse policy. One code, three
/// severities: the conversion policy decides whether the formula fails the conversion,
/// is omitted, or is kept as source.
pub const MATH_UNSUPPORTED: Message = Message {
    code: "typst.math.unsupported",
    severity: Severity::Error,
    status: Status::Unsupported,
    title: "MATH NOT TRANSLATABLE TO TYPST",
    message: "I cannot translate {0} in this LaTeX formula to Typst math.",
    hint: "Rewrite the formula with supported commands, or set the math policy to omit or \
           allow to leave it out or keep its LaTeX source as text.",
};

/// Math the adapter cannot translate, under the omit policy.
pub const MATH_OMITTED: Message = Message {
    code: "typst.math.unsupported",
    severity: Severity::Warning,
    status: Status::Unsupported,
    title: "MATH OMITTED FROM TYPST OUTPUT",
    message: "I cannot translate {0} in this LaTeX formula, so the formula is omitted.",
    hint: "Rewrite the formula with supported commands to keep it in the output.",
};

/// Math the adapter cannot translate, under the allow policy: kept as LaTeX source.
pub const MATH_VERBATIM: Message = Message {
    code: "typst.math.unsupported",
    severity: Severity::Warning,
    status: Status::Unsupported,
    title: "MATH KEPT AS LATEX SOURCE",
    message: "I cannot translate {0} in this LaTeX formula; its source is shown as text.",
    hint: "Rewrite the formula with supported commands to typeset it.",
};

/// Raw Typst under the refuse policy.
pub const RAW_REFUSED: Message = Message {
    code: "typst.raw.refused",
    severity: Severity::Error,
    status: Status::Policy,
    title: "RAW TYPST REFUSED",
    message: "This raw Typst content would run as code, and the policy refuses raw content.",
    hint: "Use a portable construct, or pass --raw=allow for trusted input or --raw=omit to \
           leave it out.",
};

/// Raw Typst under the omit policy.
pub const RAW_OMITTED: Message = Message {
    code: "typst.raw.omitted",
    severity: Severity::Warning,
    status: Status::Policy,
    title: "RAW TYPST OMITTED",
    message: "This raw Typst content was left out because the policy omits raw content.",
    hint: "Use a portable construct, or pass --raw=allow for trusted input.",
};

/// Raw content for another format, under the refuse policy.
pub const RAW_FOREIGN: Message = Message {
    code: "typst.raw.foreign",
    severity: Severity::Error,
    status: Status::Unsupported,
    title: "RAW CONTENT FOR ANOTHER FORMAT",
    message: "This raw {0} content cannot be expressed in Typst.",
    hint: "Use a portable construct, add a raw typst alternative, or pass --raw=omit to \
           leave it out.",
};

/// Raw content for another format, under the omit or allow policy: never written.
pub const RAW_FOREIGN_OMITTED: Message = Message {
    code: "typst.raw.foreign",
    severity: Severity::Warning,
    status: Status::Unsupported,
    title: "RAW CONTENT FOR ANOTHER FORMAT OMITTED",
    message: "This raw {0} content cannot be expressed in Typst and was left out.",
    hint: "Use a portable construct, or add a raw typst alternative.",
};

/// A reference nothing resolved.
pub const LINK_UNRESOLVED: Message = Message {
    code: "typst.link.unresolved",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "UNRESOLVED REFERENCE",
    message: "The reference {0} has no target.",
    hint: "Define the target, or correct the reference name.",
};

/// A `javascript:`, `vbscript:`, or non-image `data:` link under the refuse policy.
pub const URL_UNSAFE: Message = Message {
    code: "typst.url.unsafe",
    severity: Severity::Error,
    status: Status::Policy,
    title: "UNSAFE LINK REFUSED",
    message: "The link {0} uses an executable or data scheme.",
    hint: "Use an http, https, or relative URL, or set the URL policy to omit to keep only \
           the link text.",
};

/// An unsafe link under the omit policy: only its text is kept.
pub const URL_OMITTED: Message = Message {
    code: "typst.url.unsafe",
    severity: Severity::Warning,
    status: Status::Policy,
    title: "UNSAFE LINK OMITTED",
    message: "The link {0} uses an executable or data scheme; only its text is kept.",
    hint: "Use an http, https, or relative URL.",
};

/// A remote image, which Typst cannot embed.
pub const IMAGE_REMOTE: Message = Message {
    code: "typst.image.remote",
    severity: Severity::Warning,
    status: Status::Unsupported,
    title: "REMOTE IMAGE SHOWN AS A LINK",
    message: "Typst embeds only local files, so the image {0} is shown as a link.",
    hint: "Download the image next to the document and refer to it by a relative path.",
};

/// An image size that is not a length.
pub const DIMENSION_IGNORED: Message = Message {
    code: "typst.image.dimension",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "IMAGE SIZE IGNORED",
    message: "The size {0} is not a length Typst understands, so it was ignored.",
    hint: "Use a number with px, pt, mm, cm, in, em, or %.",
};

/// A footnote reference without a footnote.
pub const FOOTNOTE_MISSING: Message = Message {
    code: "typst.footnote.missing",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "FOOTNOTE NOT FOUND",
    message: "The footnote reference {0} has no footnote; its label is shown as text.",
    hint: "Add the footnote, or correct the reference label.",
};

/// A citation reference without a citation.
pub const CITATION_MISSING: Message = Message {
    code: "typst.citation.missing",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "CITATION NOT FOUND",
    message: "The citation reference {0} has no citation; its label is shown as text.",
    hint: "Add the citation, or correct the reference label.",
};

/// A substitution without a definition, or one that refers to itself.
pub const SUBSTITUTION_MISSING: Message = Message {
    code: "typst.substitution.missing",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "SUBSTITUTION NOT EXPANDED",
    message: "The substitution {0} has no definition, or refers to itself.",
    hint: "Define the substitution once, without referring to itself.",
};

/// `RenderOptions::language` is not a language tag.
pub const LANGUAGE_INVALID: Message = Message {
    code: "typst.language.invalid",
    severity: Severity::Warning,
    status: Status::InvalidInput,
    title: "LANGUAGE TAG IGNORED",
    message: "The language tag {0} is not a two- or three-letter ISO 639 code.",
    hint: "Use a tag such as en, de, or pt-BR; English is used instead.",
};

/// Rendering recursion past the depth bound.
pub const DEPTH_LIMIT: Message = Message {
    code: "typst.limit.depth",
    severity: Severity::Error,
    status: Status::Limit,
    title: "CONTENT NESTED TOO DEEPLY FOR TYPST",
    message: "Rendering this content needs more than {0} nested levels.",
    hint: "Flatten the nesting, or raise limits.max_depth.",
};
