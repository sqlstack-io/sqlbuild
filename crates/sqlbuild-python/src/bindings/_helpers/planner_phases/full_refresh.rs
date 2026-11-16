//! Per-model full-refresh resolution and full-rebuild refusals.

use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::model_config::typed_config::{config_value, model_values};
use crate::bindings::_helpers::planner_phases::constants::FULL_REFRESH_DISABLED_REBUILD_CODE;
use crate::bindings::_helpers::planner_phases::objects::{frozenset, planner_error, strings};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyBool;
use sqlbuild_analysis::planner_warehouse::main::full_rebuild_cause::full_rebuild_cause;
use sqlbuild_analysis::planner_warehouse::models::FullRebuildFacts;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const FULL_REFRESH_KEY: &str = "full_refresh";

fn configured_full_refresh<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    config_value(model.py(), &model_values(model)?, FULL_REFRESH_KEY)
}

/// dbt-compatible nullable model full-refresh semantics.
pub(crate) fn resolve_model_full_refresh(
    model: &Bound<'_, PyAny>,
    cli_full_refresh: bool,
) -> PyResult<bool> {
    let configured = configured_full_refresh(model)?;
    if configured.is_instance_of::<PyBool>() {
        return configured.extract();
    }
    Ok(cli_full_refresh)
}

/// Names of models effectively full refreshed for this invocation.
pub(crate) fn effectively_full_refreshed_model_names<'py>(
    project: &Bound<'py, PyAny>,
    cli_full_refresh: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let mut names: Vec<_> = Vec::new();
    for model in project_reads::models(project)?.try_iter()? {
        let model = model?;
        if resolve_model_full_refresh(&model, cli_full_refresh)? {
            names.push(model_reads::name_object(&model)?);
        }
    }
    frozenset(project.py(), &names)
}

#[pyfunction]
#[pyo3(signature = (*, change_result, full_refresh))]
pub(crate) fn describe_full_rebuild_cause(
    change_result: &Bound<'_, PyAny>,
    full_refresh: bool,
) -> PyResult<String> {
    Ok(full_rebuild_cause(&FullRebuildFacts {
        full_refresh,
        change_kind: native_field(change_result, "change_kind")?
            .str()?
            .extract::<String>()?,
        query_changed: native_field(change_result, "query_changed")?.is_truthy()?,
        changed_functions: strings(&native_field(change_result, "changed_functions")?)?,
        has_schema_findings: native_field(change_result, "schema_findings")?.is_truthy()?,
    }))
}

#[pyfunction]
#[pyo3(signature = (*, model, change_result, full_rebuild_cause))]
pub(crate) fn check_full_rebuild_allowed(
    model: &Bound<'_, PyAny>,
    change_result: &Bound<'_, PyAny>,
    full_rebuild_cause: &str,
) -> PyResult<()> {
    let py = model.py();
    if !configured_full_refresh(model)?.is(PyBool::new(py, false)) {
        return Ok(());
    }
    if native_field(change_result, "change_kind")?.eq("first_run")?
        || native_field(change_result, "query_changed")?.is_truthy()?
    {
        return Ok(());
    }
    let name: String = model_reads::model_name(model)?;
    Err(planner_error(
        py,
        &format!(
            "model '{name}' sets full_refresh false, but the plan would fully rebuild it ({full_rebuild_cause})"
        ),
        (
            Some(FULL_REFRESH_DISABLED_REBUILD_CODE),
            Some(
                "set replay_on_change to forward or bounded-<duration> on this model to keep its existing history, or remove full_refresh false to allow the rebuild",
            ),
        ),
    ))
}
