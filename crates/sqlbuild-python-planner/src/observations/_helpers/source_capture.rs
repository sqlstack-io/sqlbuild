//! The expression-source columns a build's planning described, kept for its observations.

use std::cell::RefCell;

/// Each described source's name and observation value, in the order planning described them.
pub(crate) type DescribedSources = Vec<(String, String)>;

thread_local! {
    /// Per thread, since a build plans and records on the thread that runs it; `None` when not armed.
    pub(crate) static CAPTURED: RefCell<Option<DescribedSources>> = const { RefCell::new(None) };
}
