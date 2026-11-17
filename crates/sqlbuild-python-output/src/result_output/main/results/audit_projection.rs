//! The audit-history projection counts, flagged as degraded when any write failed.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyList;
use sqlbuild_python_planner::plan_output::main::any::any;
use sqlbuild_python_planner::plan_output::main::dict::dict;

/// The audit-history projection counts, flagged as degraded when any write failed.
pub fn audit_projection<'py>(
    py: Python<'py>,
    (attempted, written, failed): (i64, i64, i64),
) -> PyResult<Vec<(&'static str, Bound<'py, PyAny>)>> {
    let counts: Vec<(&str, Bound<'py, PyAny>)> = [
        ("attempted_count", attempted),
        ("written_count", written),
        ("failed_count", failed),
    ]
    .into_iter()
    .map(|(key, value)| Ok((key, any(py, value)?)))
    .collect::<PyResult<_>>()?;
    let failed: bool = counts[2].1.is_truthy()?;
    let mut entries: Vec<(&'static str, Bound<'py, PyAny>)> = vec![(
        "audit_result_projection",
        dict(py, counts.clone())?.into_any(),
    )];
    if failed {
        let mut reason: Vec<(&str, Bound<'py, PyAny>)> =
            vec![("reason", any(py, "audit_result_projection_failure")?)];
        reason.extend(counts);
        entries.push(("projection_degraded", any(py, true)?));
        entries.push((
            "projection_degradation_reasons",
            PyList::new(py, [dict(py, reason)?])?.into_any(),
        ));
    }
    Ok(entries)
}
