//! The reference one named input resolves to through model and SQL function references.

use crate::planner_strategy::_helpers::lineage;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_python_project::native_host::models::ModelLookup;

/// The reference one named input resolves to through model and SQL function references.
#[inline]
pub fn resolve_lineage_reference<'py>(
    model: &Bound<'py, PyAny>,
    input_name: &str,
    models_by_name: ModelLookup,
    functions_by_name: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    lineage::resolve_lineage_reference(model, input_name, models_by_name, functions_by_name)
}
