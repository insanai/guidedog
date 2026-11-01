//! Conservative live-allocation allowance, including repeated text and state copies.

use std::cell::Cell;

use gd_score::{Annotation, AnnotationKind, Span};

use crate::Error;
use crate::interpret::{Reader, State};
use crate::limits::limit;

#[derive(Debug)]
pub(crate) struct Memory {
    left: Cell<usize>,
}

impl Memory {
    pub(crate) fn new(bytes: usize) -> Self {
        Self { left: Cell::new(bytes) }
    }

    pub(crate) fn take(&self, bytes: usize, span: Span) -> Result<(), Error> {
        let left = self.left.get().checked_sub(bytes).ok_or_else(|| limit("memory", span))?;
        self.left.set(left);
        Ok(())
    }
}

impl Reader<'_> {
    pub(crate) fn copy_text(&self, text: &str, span: Span) -> Result<String, Error> {
        self.memory.take(text.len().saturating_add(32), span)?;
        let mut result = String::new();
        result.try_reserve_exact(text.len()).map_err(|_| limit("memory", span))?;
        result.push_str(text);
        Ok(result)
    }

    pub(crate) fn copy_state(&self, state: &State, span: Span) -> Result<State, Error> {
        let bytes = state.transpositions.len() * 32
            + state.last_chord.len() * 32
            + state.ties.len() * 16
            + state.slurs.len() * 64
            + state.pending.iter().map(annotation_bytes).sum::<usize>()
            + 1024;
        self.memory.take(bytes, span)?;
        Ok(state.clone())
    }
}

pub(crate) fn annotation_bytes(annotation: &Annotation) -> usize {
    let text = match &annotation.kind {
        AnnotationKind::Text(text, _)
        | AnnotationKind::Articulation(text)
        | AnnotationKind::Dynamic(text)
        | AnnotationKind::Tempo(text)
        | AnnotationKind::Rehearsal(text) => text.len(),
        AnnotationKind::Markup(markup, _) => return markup.approx_bytes(),
        AnnotationKind::Fingering(_) => 0,
    };
    text.saturating_add(128)
}
