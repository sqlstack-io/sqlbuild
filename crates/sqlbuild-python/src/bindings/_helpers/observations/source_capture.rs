//! The expression-source columns a build's planning described, kept for its observations.

use std::cell::RefCell;

/// Each described source's name and column names, in the order planning described them.
pub(crate) type DescribedSources = Vec<(String, Vec<String>)>;

thread_local! {
    /// Per thread, since a build plans and records on the thread that runs it; `None` when not armed.
    static CAPTURED: RefCell<Option<DescribedSources>> = const { RefCell::new(None) };
}

/// Start capturing described source columns; only a build arms it, so `sqb plan` records nothing.
pub(crate) fn arm_source_capture() {
    CAPTURED.with(|slot| *slot.borrow_mut() = Some(Vec::new()));
}

/// Keep one source's described column names when a build armed the capture.
pub(crate) fn capture_source_columns(source: &str, columns: &[String]) {
    CAPTURED.with(|slot| {
        if let Some(captured) = slot.borrow_mut().as_mut() {
            captured.push((source.to_owned(), columns.to_vec()));
        }
    });
}

/// The captured source columns, disarming the capture.
pub(crate) fn take_source_capture() -> DescribedSources {
    CAPTURED
        .with(|slot| slot.borrow_mut().take())
        .unwrap_or_default()
}
