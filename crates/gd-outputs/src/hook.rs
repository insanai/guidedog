//! What a test makes a commit step do (publish.odin's `commit_hook`): go on, stop as
//! if the process died there, or fail as if the system refused it. Outside this
//! crate's tests every step goes on.

/// What a commit step does. Only tests stop or fail a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum StepAction {
    /// Carry the step out.
    Go,
    /// Stop the commit before the step, as a crash would.
    Crash,
    /// Fail the step, as a refusal of the system would.
    Fail,
}

/// What step `step` (counted from 1 among the steps that change what is published)
/// does.
#[cfg(not(test))]
pub(crate) fn step_action(_step: usize) -> StepAction {
    StepAction::Go
}

/// Whether a commit switches the folders in two renames even where the system can
/// exchange them.
#[cfg(not(test))]
pub(crate) fn renames_only() -> bool {
    false
}

#[cfg(test)]
pub(crate) use testing::{renames_only, set_hook, set_renames_only, step_action};

#[cfg(test)]
mod testing {
    use std::cell::{Cell, RefCell};

    use super::StepAction;

    type Hook = Box<dyn FnMut(usize) -> StepAction>;

    thread_local! {
        static HOOK: RefCell<Option<Hook>> = RefCell::new(None);
        static RENAMES_ONLY: Cell<bool> = const { Cell::new(false) };
    }

    pub(crate) fn step_action(step: usize) -> StepAction {
        HOOK.with(|hook| hook.borrow_mut().as_mut().map_or(StepAction::Go, |h| h(step)))
    }

    pub(crate) fn renames_only() -> bool {
        RENAMES_ONLY.with(Cell::get)
    }

    /// Sets the hook of this thread's commits; `None` clears it.
    pub(crate) fn set_hook(hook: Option<Hook>) {
        HOOK.with(|h| *h.borrow_mut() = hook);
    }

    /// Makes this thread's commits switch folders in two renames.
    pub(crate) fn set_renames_only(on: bool) {
        RENAMES_ONLY.with(|r| r.set(on));
    }
}
