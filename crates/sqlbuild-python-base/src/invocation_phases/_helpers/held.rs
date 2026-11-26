//! The planned phase this thread finished, until the command knows everything its plan runs.

use std::cell::RefCell;

use crate::invocation_phases::models::HeldPhase;

thread_local! {
    pub(crate) static HELD_PLANNED: RefCell<Option<HeldPhase>> = const { RefCell::new(None) };
}
