//! Apply CLI, project, and model batch limits in their documented precedence order.

use crate::planner_strategy::_helpers::microbatch_limits::default_limit;
use crate::planner_strategy::_helpers::microbatch_limits::limited;
use crate::planner_strategy::_helpers::microbatch_limits::with_warnings;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyTuple;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::compiled_views::main::project::replace::replace;

/// Apply CLI, project, and model batch limits in their documented precedence order.
#[pyfunction]
#[pyo3(signature = (*, entry, inputs))]
pub fn apply_configured_microbatch_limits<'py>(
    entry: &Bound<'py, PyAny>,
    inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = entry.py();
    let configured: Bound<'py, PyAny> = native_field(inputs, "max_microbatches")?;
    let configured_action: Bound<'py, PyAny> = native_field(inputs, "microbatch_limit_action")?;
    if native_field(inputs, "max_microbatches_is_override")?.is_truthy()? {
        let error: Bound<'py, PyAny> = enum_member(py, RecordId::MicrobatchLimitAction, "ERROR")?;
        return with_warnings(limited(entry, configured, error)?, Vec::new());
    }
    let declared: Bound<'py, PyAny> = native_field(entry, "declared_microbatch_limit_action")?;
    if declared.is_none() {
        let maximum: Bound<'py, PyAny> = default_limit(entry, configured)?;
        return with_warnings(limited(entry, maximum, configured_action)?, Vec::new());
    }
    let mut warnings: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut entry: Bound<'py, PyAny> = entry.clone();
    if !configured.is_none() {
        let (_, safety_warning) = limited(&entry, configured.clone(), configured_action.clone())?;
        entry = replace(
            &entry,
            &[
                ("microbatch_safety_limit", configured),
                ("microbatch_safety_limit_action", configured_action),
            ],
        )?;
        warnings.extend(safety_warning);
    }
    let model_limit: Bound<'py, PyAny> = native_field(&entry, "microbatch_limit")?;
    with_warnings(limited(&entry, model_limit, declared)?, warnings)
}
