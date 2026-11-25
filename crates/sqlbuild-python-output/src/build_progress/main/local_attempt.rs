//! In `auto`, local attempts that fall back to the warehouse leave no progress behind.

use crate::build_progress::main::test_label::test_label;
use crate::build_progress::models::{LocalAttemptStep, NestedCommandProgressCallbacks};
use crate::result_output::main::events::sql_test_identity::sql_test_identity;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::build_progress::main::progress::current_projector::current_projector;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Hold, release, abandon or settle local attempt progress; whether a settled replay fell back.
pub fn local_attempt(step: LocalAttemptStep<'_>) -> PyResult<bool> {
    match step {
        LocalAttemptStep::Hold(progress, hold) => {
            if let (true, Ok(reporter)) = (hold, progress.cast::<NestedCommandProgressCallbacks>())
            {
                reporter.get().hold_items(progress.py())?;
            }
        }
        LocalAttemptStep::Release(progress) => {
            if let Ok(reporter) = progress.cast::<NestedCommandProgressCallbacks>() {
                reporter.get().release_items(progress.py())?;
            }
        }
        LocalAttemptStep::Abandon(progress, result) => {
            if let Ok(reporter) = progress.cast::<NestedCommandProgressCallbacks>() {
                NestedCommandProgressCallbacks::on_item_abandoned(
                    reporter,
                    &test_label(result, "test_name")?,
                    &sql_test_identity(result, "test_name")?,
                )?;
            }
        }
        LocalAttemptStep::HoldReplays(py, hold) => {
            if let (true, Some(projector)) = (hold, current_projector(py)?) {
                projector.get().hold_lines();
            }
        }
        LocalAttemptStep::ReleaseReplays(py) => {
            if let Some(projector) = current_projector(py)? {
                let held: Vec<String> = projector.get().release_lines();
                projector.get().write_held(py, &held)?;
            }
        }
        LocalAttemptStep::SettleReplay(result) => return settle_replay(result),
    }
    Ok(false)
}

/// Drop a held replay's lines and terminal when it falls back; else write them. Whether it fell back.
fn settle_replay(result: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py = result.py();
    let Some(projector) = current_projector(py)? else {
        return Ok(false);
    };
    let projector = projector.get();
    if !projector.is_holding() {
        return Ok(false);
    }
    let held: Vec<String> = projector.exchange_held_lines(Vec::new());
    if native_field(result, "local_refusal")?.is_none() {
        projector.write_held(py, &held)?;
        return Ok(false);
    }
    projector
        .forget_resource_named(&native_field(result, "scenario_name")?.extract::<String>()?)?;
    Ok(true)
}
