use crate::{Error, ErrorKind, Span};

/// Bounds for one load, including includes, variable expansion and repeats.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Source bytes, charging repeated includes each time.
    pub source_bytes: usize,
    /// Recursive syntax and expansion depth.
    pub depth: usize,
    /// Included source files, excluding the main source.
    pub files: usize,
    /// Token and syntax-node visits, including expansions.
    pub work: usize,
    /// Rhythmic and notation events after expansion.
    pub events: usize,
    /// Conservative allocation allowance for expanded notation, including copied text.
    pub memory_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            source_bytes: 256 * 1024,
            depth: 64,
            files: 32,
            work: 8_000_000,
            events: 20_000,
            memory_bytes: 64 * 1024 * 1024,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Work {
    left: usize,
}

impl Work {
    pub(crate) fn new(limits: &Limits) -> Self {
        Self { left: limits.work }
    }

    pub(crate) fn take(&mut self, count: usize, span: Span) -> Result<(), Error> {
        self.left = self.left.checked_sub(count).ok_or_else(|| limit("reader work", span))?;
        Ok(())
    }
}

pub(crate) fn limit(what: &str, span: Span) -> Error {
    Error::new(ErrorKind::ReadLimit, format!("The score exceeds the {what} limit."))
        .at(span)
        .hint("Split the example into smaller scores or reduce nested expansion.")
}

pub(crate) fn push<T>(items: &mut Vec<T>, value: T, span: Span) -> Result<(), Error> {
    items.try_reserve(1).map_err(|_| limit("memory", span))?;
    items.push(value);
    Ok(())
}
