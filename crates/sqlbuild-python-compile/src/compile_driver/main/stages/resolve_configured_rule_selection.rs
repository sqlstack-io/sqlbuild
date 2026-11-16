//! Align configured-rule subjects with models that received deep SQL analysis.

use crate::compile_driver::_helpers::setup::pipeline_entries::compile_analysis_selection;
use crate::compile_driver::_helpers::stages::selection_phases::analysis_scope;
use crate::compile_driver::_helpers::stages::selection_phases::planner_scopes;
use crate::compile_driver::_helpers::stages::selection_phases::unselective;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyFrozenSet;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Align configured-rule subjects with models that received deep SQL analysis.
#[pyfunction]
#[pyo3(signature = (*, options, discovered_inputs, project))]
pub fn resolve_configured_rule_selection<'py>(
    options: &Bound<'py, PyAny>,
    discovered_inputs: &Bound<'py, PyAny>,
    project: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = options.py();
    if !native_field(options, "sql_test_selection")?
        .getattr("models_selected")?
        .is_truthy()?
    {
        return Ok(PyFrozenSet::empty(py)?.into_any());
    }
    let selection: Bound<'py, PyAny> = compile_analysis_selection(options, discovered_inputs)?;
    if selection.is_none() || unselective(&selection)? {
        return Ok(py.None().into_bound(py));
    }
    let scope = analysis_scope(&planner_scopes(project, &selection)?)?;
    let keys: Vec<Bound<'py, PyAny>> = scope
        .models
        .values()
        .map(|model| native_field(&model.bind(py), "key"))
        .collect::<PyResult<_>>()?;
    Ok(PyFrozenSet::new(py, keys)?.into_any())
}
