//! Included and added sources. A reader asks the caller's resource provider for an
//! included file; the core checks policy, depth, cycles, and the byte budget, and
//! records the text in the workspace's source table so spans into it stay meaningful.

use std::sync::Arc;

use crate::adapter::ReadContext;
use crate::options::{Resource, ResourceStatus};
use crate::report::{Arg, Message, Severity, arg_int, arg_span};
use crate::snapshot::{SourceEntry, SourceUse};
use crate::types::{Region, SourceId, SourceSpan, Status};

/// Includes are disabled by the policy.
const INCLUDE_DISABLED: Message = Message {
    code: "core.include.disabled",
    severity: Severity::Error,
    status: Status::Policy,
    title: "INCLUDES ARE DISABLED",
    message: "This document asks to include {0}, but the policy disallows includes.",
    hint: "Inline the content, or enable includes in the conversion policy.",
};

/// The caller supplied no resource provider.
const INCLUDE_NO_PROVIDER: Message = Message {
    code: "core.include.no-provider",
    severity: Severity::Error,
    status: Status::Policy,
    title: "NO RESOURCE PROVIDER",
    message: "This document includes {0}, but the caller supplied no resource provider.",
    hint: "Convert through a host that provides files, such as guidedog convert.",
};

/// The provider has no such resource. Readers report it too (literalinclude).
pub const INCLUDE_MISSING: Message = Message {
    code: "core.include.missing",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INCLUDED FILE NOT FOUND",
    message: "I could not find {0}.",
    hint: "Check the path; it is relative to the file that includes it.",
};

/// The provider refused the resource.
const INCLUDE_REFUSED: Message = Message {
    code: "core.include.refused",
    severity: Severity::Error,
    status: Status::Policy,
    title: "INCLUDE REFUSED",
    message: "The host refused to provide {0}.",
    hint: "Keep included files inside the project's approved source roots.",
};

/// The provider's memory budget cannot hold the resource.
const INCLUDE_LIMIT: Message = Message {
    code: "core.include.limit",
    severity: Severity::Error,
    status: Status::Limit,
    title: "INCLUDE OVER BUDGET",
    message: "The host's memory budget cannot hold {0}.",
    hint: "Include smaller files, or increase the memory budget of the host's resource provider.",
};

/// The provided resource is not UTF-8 text.
const INCLUDE_NOT_UTF8: Message = Message {
    code: "core.include.not-utf8",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INCLUDED FILE IS NOT UTF-8",
    message: "I found {0}, but its text is not UTF-8.",
    hint: "Convert the file to UTF-8, for example with `iconv -t UTF-8`, and build again.",
};

/// A source includes itself.
const INCLUDE_CYCLE: Message = Message {
    code: "core.include.cycle",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INCLUDE CYCLE",
    message: "{0} includes itself through this directive.",
    hint: "Remove one of the include directives that form the cycle.",
};

/// Includes nest past `Limits::max_include_depth`.
const INCLUDE_DEPTH: Message = Message {
    code: "core.include.depth",
    severity: Severity::Error,
    status: Status::Limit,
    title: "INCLUDES NESTED TOO DEEPLY",
    message: "Includes are nested more than {0} levels deep.",
    hint: "Flatten the include structure, or raise limits.max_include_depth.",
};

/// Included text passes `Limits::max_included_bytes`.
const INCLUDE_BYTES: Message = Message {
    code: "core.include.bytes",
    severity: Severity::Error,
    status: Status::Limit,
    title: "TOO MUCH INCLUDED TEXT",
    message: "Included sources exceed {0} bytes in total.",
    hint: "Split the document, or raise limits.max_included_bytes.",
};

fn int_arg(v: usize) -> Arg {
    arg_int(i64::try_from(v).unwrap_or(i64::MAX))
}

impl ReadContext<'_> {
    /// Asks the resource provider for `name`, which a directive at `span` in source
    /// `from` names, records it in the source table, and returns its id. Every refusal
    /// is reported at `span` and gives `None`; running out of sources poisons the
    /// builder. `usage` tells whether the reader parses the text or shows it as it is.
    pub fn include_source(
        &mut self,
        from: SourceId,
        name: &str,
        span: SourceSpan,
        usage: SourceUse,
    ) -> Option<SourceId> {
        let quoted = arg_span(span);
        if !self.policy.includes {
            self.report(INCLUDE_DISABLED, span, quoted, Arg::None);
            return None;
        }
        let Some(provider) = self.resources else {
            self.report(INCLUDE_NO_PROVIDER, span, quoted, Arg::None);
            return None;
        };
        if self.include_depth(from) >= self.limits.max_include_depth {
            self.report(INCLUDE_DEPTH, span, int_arg(self.limits.max_include_depth), Arg::None);
            return None;
        }
        let from_name = self.sources.get(from.index()).map_or("", |e| e.name.as_str());
        let resource = match provider.provide(from_name, name) {
            Ok(resource) => resource,
            Err(status) => {
                let m = match status {
                    ResourceStatus::Missing => INCLUDE_MISSING,
                    ResourceStatus::Refused => INCLUDE_REFUSED,
                    ResourceStatus::Limit => INCLUDE_LIMIT,
                    ResourceStatus::NotUtf8 => INCLUDE_NOT_UTF8,
                };
                self.report(m, span, quoted, Arg::None);
                return None;
            }
        };
        self.record_include(from, resource, span, usage)
    }

    /// Counts the includes between `from` and the main source.
    fn include_depth(&self, from: SourceId) -> usize {
        let mut depth = 0;
        let mut id = from;
        while id != SourceId::MAIN && depth <= self.sources.len() {
            depth += 1;
            id = self.sources.get(id.index()).map_or(SourceId::MAIN, |e| e.parent);
        }
        depth
    }

    /// Tells whether `canonical` names `from` or a source that includes it.
    fn includes_itself(&self, from: SourceId, canonical: &str) -> bool {
        let mut id = from;
        for _ in 0..=self.sources.len() {
            let Some(entry) = self.sources.get(id.index()) else {
                return false;
            };
            if entry.name == canonical {
                return true;
            }
            if id == SourceId::MAIN {
                return false;
            }
            id = entry.parent;
        }
        false
    }

    /// Checks a provided resource against cycles and budgets and adds it to the table.
    fn record_include(
        &mut self,
        from: SourceId,
        resource: Resource,
        span: SourceSpan,
        usage: SourceUse,
    ) -> Option<SourceId> {
        if self.includes_itself(from, &resource.canonical) {
            self.report(INCLUDE_CYCLE, span, arg_span(span), Arg::None);
            return None;
        }
        let total = self.included.saturating_add(resource.text.len());
        if total > self.limits.max_included_bytes {
            self.report(INCLUDE_BYTES, span, int_arg(self.limits.max_included_bytes), Arg::None);
            return None;
        }
        let mut entry = SourceEntry::shared(resource.canonical, resource.text, from);
        entry.included_at = span;
        entry.usage = usage;
        let id = self.push_source(entry)?;
        self.included = total;
        Some(id)
    }

    /// Appends an entry to the source table; a full table poisons the builder.
    fn push_source(&mut self, entry: SourceEntry) -> Option<SourceId> {
        let Ok(id) = u16::try_from(self.sources.len()) else {
            let _ = self.b.capacity(Region::Sources, 1);
            return None;
        };
        if id == u16::MAX || self.sources.try_reserve(1).is_err() {
            let _ = self.b.capacity(Region::Sources, 1);
            return None;
        }
        self.sources.push(entry);
        Some(SourceId(id))
    }

    /// Registers text that is not a file, such as a configured prologue, so nodes parsed
    /// from it carry spans into it. The text is copied (shared) into the table.
    pub fn add_source(&mut self, name: &str, text: &str) -> Option<SourceId> {
        let entry = SourceEntry::shared(name.to_owned(), Arc::from(text), SourceId::MAIN);
        self.push_source(entry)
    }
}
