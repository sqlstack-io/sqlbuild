//! The reference one named input resolves to through model and SQL function references.

use crate::planner_strategy::_helpers::lineage::ProjectLineage;
use crate::planner_strategy::_helpers::lineage::lineage_error;
use crate::planner_strategy::_helpers::lineage::references;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_analysis::planner_strategy::main::lineage_reference::lineage_reference;
use sqlbuild_python_project::compiled_views::main::compiled_models::model_name::model_name;
use sqlbuild_python_project::native_host::models::ModelLookup;

/// The reference one named input resolves to through model and SQL function references.
pub fn resolve_lineage_reference<'py>(
    model: &Bound<'py, PyAny>,
    input_name: &str,
    models_by_name: ModelLookup,
    functions_by_name: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    let graph = ProjectLineage {
        models_by_name,
        functions_by_name,
    };
    let name: String = model_name(model)?;
    lineage_reference(&graph, (&name, references(model)?), input_name)
        .map_err(|failure| lineage_error(py, failure))
}
