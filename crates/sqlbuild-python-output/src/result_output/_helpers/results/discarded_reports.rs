//! Whether the running command's `--json` report is discarded, as a worker's run discards it.

use std::cell::Cell;

thread_local! {
    /// Set for the worker's run, whose printed output is human output only.
    pub(crate) static REPORTS_DISCARDED: Cell<bool> = const { Cell::new(false) };
}
