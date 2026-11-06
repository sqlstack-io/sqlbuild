//! Unit-test selector splitting, analysis and rule selection, and static command compilation.

use std::sync::Arc;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyFrozenSet;
use pyo3::{pyfunction, wrap_pyfunction};

use crate::bindings::_helpers::boundary::python_calls::{imported, keywords};
use crate::bindings::_helpers::compile_driver::assembly::assemble_project;
use crate::bindings::_helpers::compile_driver::pipeline_entries::compile_analysis_selection;
use crate::bindings::_helpers::planner_phases::scopes::resolve_planner_scopes;
use crate::bindings::_helpers::planner_scope::retained::{ScopeData, scope_data};
use crate::bindings::_helpers::planner_selection::unit_tests::split_project_unit_test_selectors;
use crate::bindings::_helpers::records::construct::{new_record, new_record_with};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;

const DATACLASSES_MODULE: &str = "dataclasses";

/// Whether a selection names nothing to select or exclude.
fn unselective(selection: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(!native_field(selection, "select")?.is_truthy()?
        && !native_field(selection, "exclude")?.is_truthy()?)
}

/// Options whose selectors name only resources, with unit tests selected apart.
pub(crate) fn unit_test_selectors_applied<'py>(
    options: &Bound<'py, PyAny>,
    discovered: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    let resolved: Bound<'py, PyAny> = if options.is_none() {
        new_record(py, RecordId::CompilePipelineOptions, (), None)?
    } else {
        options.clone()
    };
    if unselective(&resolved)? {
        return Ok(resolved);
    }
    let split: Bound<'py, PyAny> = split_project_unit_test_selectors(
        &native_field(&resolved, "select")?,
        &native_field(&resolved, "exclude")?,
        discovered,
        native_field(&resolved, "accepts_unit_test_selectors")?.extract()?,
    )?;
    imported(py, DATACLASSES_MODULE, "replace")?.call(
        (resolved,),
        Some(&keywords(
            py,
            &[
                ("select", native_field(&split, "select")?),
                ("exclude", native_field(&split, "exclude")?),
                (
                    "sql_test_selection",
                    native_field(&split, "sql_test_selection")?,
                ),
            ],
        )?),
    )
}

/// The planner scopes `selection` resolves over `project`.
fn planner_scopes<'py>(
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
fn analysis_scope(scopes: &Bound<'_, PyAny>) -> PyResult<Arc<ScopeData>> {
    scope_data(&native_field(scopes, "stale_warning_scope")?)
}

/// Return options whose selectors name only resources, with unit tests selected apart.
#[pyfunction]
#[pyo3(signature = (*, options, discovered_inputs))]
pub(crate) fn apply_unit_test_selectors<'py>(
    options: &Bound<'py, PyAny>,
    discovered_inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    unit_test_selectors_applied(options, discovered_inputs)
}

/// Align configured-rule subjects with models that received deep SQL analysis.
#[pyfunction]
#[pyo3(signature = (*, options, discovered_inputs, project))]
pub(crate) fn resolve_configured_rule_selection<'py>(
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(apply_unit_test_selectors, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_configured_rule_selection, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_analysis_model_names, module)?)?;
    Ok(())
}
