//! Model header `values` with `${name}`, `${ENV:...}` and early `run.*` templates resolved.

use crate::model_config::_helpers::config::template_data;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Model header `values` with `${name}`, `${ENV:...}` and early `run.*` templates resolved.
#[inline]
pub fn early_model_templates<'py>(
    values: &Bound<'py, PyAny>,
    effective_vars: &Bound<'py, PyAny>,
    (target_name, run_id): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    template_data::early_model_templates(values, effective_vars, (target_name, run_id))
}
