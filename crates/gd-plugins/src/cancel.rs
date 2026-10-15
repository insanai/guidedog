//! `Cancel`: a cheap, shareable cancellation probe.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A cancellation request a caller can raise while a plugin works. The default never
/// cancels. Clones share one probe; the SDK polls it between and during requests.
///
/// Cancellation asks a plugin to stop; it cannot undo a write a target accepted.
#[derive(Clone, Default)]
pub struct Cancel {
    probe: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
}

impl Cancel {
    /// A probe that reads `flag`, which a signal handler or another thread may set.
    pub fn from_flag(flag: Arc<AtomicBool>) -> Cancel {
        Cancel { probe: Some(Arc::new(move || flag.load(Ordering::SeqCst))) }
    }

    /// A probe that calls `probe`; it must be cheap, as it is polled often.
    pub fn from_fn(probe: impl Fn() -> bool + Send + Sync + 'static) -> Cancel {
        Cancel { probe: Some(Arc::new(probe)) }
    }

    /// Whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.probe.as_ref().is_some_and(|probe| probe())
    }
}

impl fmt::Debug for Cancel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cancel").field("cancelled", &self.is_cancelled()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_and_functions_cancel() {
        assert!(!Cancel::default().is_cancelled());
        let flag = Arc::new(AtomicBool::new(false));
        let cancel = Cancel::from_flag(flag.clone());
        assert!(!cancel.is_cancelled());
        flag.store(true, Ordering::SeqCst);
        assert!(cancel.clone().is_cancelled());
        assert!(Cancel::from_fn(|| true).is_cancelled());
    }
}
