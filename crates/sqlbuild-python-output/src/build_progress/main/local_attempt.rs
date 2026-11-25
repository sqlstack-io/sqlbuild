//! In `auto`, local attempts that fall back to the warehouse leave no progress behind.

use crate::build_progress::main::test_label::test_label;
use crate::build_progress::models::{LocalAttemptStep, NestedCommandProgressCallbacks};
use crate::result_output::main::events::sql_test_identity::sql_test_identity;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Hold, release or abandon local test progress, on a reporter that can hold it.
pub fn local_attempt(progress: &Bound<'_, PyAny>, step: LocalAttemptStep<'_>) -> PyResult<()> {
    let Ok(reporter) = progress.cast::<NestedCommandProgressCallbacks>() else {
        return Ok(());
    };
    match step {
        LocalAttemptStep::Hold(false) => {}
        LocalAttemptStep::Hold(true) => reporter.get().hold_items(progress.py())?,
        LocalAttemptStep::Release => reporter.get().release_items(progress.py())?,
        LocalAttemptStep::Abandon(result) => NestedCommandProgressCallbacks::on_item_abandoned(
            reporter,
            &test_label(result, "test_name")?,
            &sql_test_identity(result, "test_name")?,
        )?,
    }
    Ok(())
}
