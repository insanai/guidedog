//! Scoped charges for temporary buffers, separate from retained-buffer growth.

use std::fmt;

use gd_score::Error;

/// Borrowed memory callbacks for transient layout buffers; empty by default.
#[derive(Clone, Copy, Default)]
pub struct Scratch<'a> {
    /// Reserves bytes before allocation; false stops the layout with a limit error.
    pub reserve: Option<&'a dyn Fn(usize) -> bool>,
    /// Returns successfully reserved bytes when the temporary buffer is freed.
    /// Without this callback the caller conservatively retains the charge.
    pub release: Option<&'a dyn Fn(usize)>,
}

impl fmt::Debug for Scratch<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scratch")
            .field("reserve", &self.reserve.is_some())
            .field("release", &self.release.is_some())
            .finish()
    }
}

#[derive(Debug, Default)]
pub(crate) struct Reservation<'a> {
    pub(crate) scratch: Scratch<'a>,
    bytes: usize,
}

impl<'a> Reservation<'a> {
    pub(crate) fn empty(scratch: Scratch<'a>) -> Self {
        Self { scratch, bytes: 0 }
    }
}

impl<'a> Scratch<'a> {
    pub(crate) fn hold(self, bytes: usize) -> Result<Reservation<'a>, Error> {
        if bytes > 0 && self.reserve.is_some_and(|reserve| !reserve(bytes)) {
            return Err(crate::limits::failure("The memory ledger refused layout scratch space."));
        }
        Ok(Reservation { scratch: self, bytes: if self.reserve.is_some() { bytes } else { 0 } })
    }
}

impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        if self.bytes > 0
            && let Some(release) = self.scratch.release
        {
            release(self.bytes);
        }
    }
}
