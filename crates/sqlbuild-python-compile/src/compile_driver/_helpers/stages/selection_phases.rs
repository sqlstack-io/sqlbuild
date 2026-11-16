//! Unit-test selector splitting, analysis and rule selection, and static command compilation.

use std::sync::Arc;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyFrozenSet;

use crate::compile_driver::_helpers::stages::assembly::assemble_project;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::planner_scope::main::resolve_planner_scopes::resolve_planner_scopes;
use sqlbuild_python_project::planner_scope::main::scope_data::scope_data;
use sqlbuild_python_project::planner_scope::models::ScopeData;

pub(crate) const DATACLASSES_MODULE: &str = "dataclasses";

/// Whether a selection names nothing to select or exclude.
pub(crate) fn unselective(selection: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(!native_field(selection, "select")?.is_truthy()?
        && !native_field(selection, "exclude")?.is_truthy()?)
}

/// The planner scopes `selection` resolves over `project`.
pub(crate) fn planner_scopes<'py>(
    project: &Bound<'py, PyAny>,
    selection: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    resolve_planner_scopes(
        project,
        &new_record_with(
            py,
            RecordId::PlannerSelection,
            &[
                ("select", native_field(selection, "select")?),
                ("exclude", native_field(selection, "exclude")?),
            ],
        )?,
        &new_record_with(
            py,
            RecordId::PlannerPolicies,
            &[(
                "auto_load_sources",
                native_field(selection, "auto_load_sources")?,
            )],
        )?,
    )
}

/// The native scope whose models receive deep analysis.
pub(crate) fn analysis_scope(scopes: &Bound<'_, PyAny>) -> PyResult<Arc<ScopeData>> {
    scope_data(&native_field(scopes, "stale_warning_scope")?)
}

/// Return the models selected for deep analysis, or None to analyse every model.
#[pyfunction]
#[pyo3(signature = (*, compile_inputs, inference_profile, selection))]
pub(crate) fn resolve_analysis_model_names<'py>(
    compile_inputs: &Bound<'py, PyAny>,
    inference_profile: &Bound<'py, PyAny>,
    selection: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = compile_inputs.py();
    if selection.is_none() || unselective(selection)? {
        return Ok(py.None().into_bound(py));
    }
    let structural_project: Bound<'py, PyAny> = assemble_project(
        py,
        Some(keywords(
            py,
            &[
                ("inputs", compile_inputs.clone()),
                ("inference_profile", inference_profile.clone()),
                (
                    "skip_column_inference",
                    pyo3::types::PyBool::new(py, true).to_owned().into_any(),
                ),
                ("analysis_model_names", PyFrozenSet::empty(py)?.into_any()),
            ],
        )?),
    )?;
    let scope = analysis_scope(&planner_scopes(&structural_project, selection)?)?;
    Ok(PyFrozenSet::new(py, scope.models.keys())?.into_any())
}
