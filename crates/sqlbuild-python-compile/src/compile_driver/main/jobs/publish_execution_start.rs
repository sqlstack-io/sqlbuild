//! Announce that a build's execution started, before any of its work runs.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::invocation_phases::constants::STARTED_EXECUTION_PHASE;
use sqlbuild_python_base::invocation_phases::models::InvocationPhase;
use sqlbuild_python_base::native_host::main::publish_invocation_phase::publish_invocation_phase;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

use crate::compile_driver::_helpers::setup::pipeline_entries::SQL_PHASE_ENTRIES;

/// Publish `started_execution` with the SQL phase's concurrency under `limit`, and its connections when it runs anything.
pub fn publish_execution_start(plan: &Bound<'_, PyAny>, limit: i64) -> PyResult<()> {
    let mut width: i64 =
        i64::try_from(native_field(plan, "execution_order")?.len()?).unwrap_or(i64::MAX);
    for entry in native_field(plan, "model_entries")?.try_iter()? {
        width = width.max(native_field(&entry?, "batch_concurrency")?.extract()?);
    }
    let concurrency: i64 = limit.max(1).min(width.max(1));
    let mut has_work: bool = false;
    for name in SQL_PHASE_ENTRIES {
        has_work |= native_field(plan, name)?.is_truthy()?;
    }
    publish_invocation_phase(
        plan.py(),
        &InvocationPhase {
            phase: STARTED_EXECUTION_PHASE,
            count: Some(concurrency),
            detail: has_work.then(|| format!("{} connections", concurrency + 1)),
            ..InvocationPhase::default()
        },
    )
}
