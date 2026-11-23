//! The findings of the MathML math writer. Each is a warning: the formula stays in the
//! page as LaTeX, in the markup a script such as MathJax typesets.

use guidedoc::{Message, Severity, Status};

/// The adapter cannot translate a construct of the formula.
pub const MATH_UNSUPPORTED: Message = Message {
    code: "mathml.math.unsupported",
    severity: Severity::Warning,
    status: Status::Unsupported,
    title: "MATH NOT WRITTEN AS MATHML",
    message: "I cannot translate {0} in this LaTeX formula to MathML; its LaTeX source is \
              shown instead.",
    hint: "Rewrite the formula with supported commands, or build without the MathML math \
           output so that a script typesets the LaTeX.",
};

/// MathML Core has no form for a construct, so writing it would change the formula.
pub const MATH_LOSSY: Message = Message {
    code: "mathml.math.lossy",
    severity: Severity::Warning,
    status: Status::Unsupported,
    title: "MATH NOT WRITTEN AS MATHML",
    message: "MathML Core cannot show part of this formula ({0}); its LaTeX source is shown \
              instead.",
    hint: "Rewrite the formula without that construct, or build without the MathML math \
           output so that a script typesets the LaTeX.",
};

/// The compiler refused the translated formula.
pub const MATH_FAILED: Message = Message {
    code: "mathml.math.failed",
    severity: Severity::Warning,
    status: Status::Unsupported,
    title: "MATH NOT WRITTEN AS MATHML",
    message: "The formula could not be typeset as MathML ({0}); its LaTeX source is shown \
              instead.",
    hint: "Check the formula; if it is correct LaTeX, report it with the formula's source.",
};

/// A limit was reached; `{0}` says which.
pub const MATH_LIMIT: Message = Message {
    code: "mathml.math.limit",
    severity: Severity::Warning,
    status: Status::Limit,
    title: "MATH LIMIT REACHED",
    message: "This formula is not written as MathML because {0}; its LaTeX source is shown \
              instead.",
    hint: "Shorten the formula or split the document.",
};

/// This build has no embedded compiler.
pub const MATH_UNAVAILABLE: Message = Message {
    code: "mathml.unavailable",
    severity: Severity::Warning,
    status: Status::Unsupported,
    title: "MATHML OUTPUT IS UNAVAILABLE",
    message: "This guidedog was built without the embedded Typst compiler, which writes \
              MathML; formulas stay LaTeX.",
    hint: "Use a release build of guidedog, or build without the MathML math output.",
};
