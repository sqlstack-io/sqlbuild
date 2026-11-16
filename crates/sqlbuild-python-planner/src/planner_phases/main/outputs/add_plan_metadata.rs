//! Add the direct-mode source freshness, remaining stale models and selection diagnostics metadata.

use crate::planner_phases::_helpers::outputs::output_assembly::freshness_metadata;
use crate::planner_phases::models::PlanOutputFields;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::planner_phases::main::sorted_names::sorted_names;
use sqlbuild_python_project::planner_phases::main::text::text;

/// Add the direct-mode source freshness, remaining stale models and selection diagnostics metadata.
pub fn add_plan_metadata<'py>(
    mut output: PlanOutputFields<'py>,
    source_freshness: &Bound<'py, PyAny>,
    policies: &Bound<'py, PyAny>,
) -> PyResult<PlanOutputFields<'py>> {
    let py = source_freshness.py();
    let diagnostics = PyDict::new(py);
    diagnostics.set_item("mode", text(py, "direct"))?;
    diagnostics.set_item("enabled", native_field(policies, "selection_diagnostics")?)?;
    output.metadata.push((
        "direct_source_freshness",
        freshness_metadata(source_freshness)?,
    ));
    output.metadata.push((
        "direct_remaining_stale_model_names",
        sorted_names(py, Vec::new())?,
    ));
    output
        .metadata
        .push(("selection_diagnostics", diagnostics.into_any()));
    Ok(output)
}
