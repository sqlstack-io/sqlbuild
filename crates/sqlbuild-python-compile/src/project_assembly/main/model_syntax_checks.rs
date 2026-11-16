//! Each model's `(sql, placeholders)` that assembly passes to syntax validation.

use crate::project_assembly::_helpers::syntax_checks::syntax_checks;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyTuple;

/// Each model's `(sql, placeholders)` that assembly passes to syntax validation.
#[pyfunction]
#[pyo3(signature = (*, model_inputs, analysis_model_names, analysis_succeeded))]
pub fn model_syntax_checks<'py>(
    model_inputs: &Bound<'py, PyAny>,
    analysis_model_names: &Bound<'py, PyAny>,
    analysis_succeeded: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let checks: Vec<Bound<'py, PyTuple>> = model_inputs
        .try_iter()?
        .map(|model_input| syntax_checks(&model_input?, analysis_model_names, analysis_succeeded))
        .collect::<PyResult<_>>()?;
    PyTuple::new(model_inputs.py(), checks)
}
