//! Check full rebuild allowed.

use crate::planner_phases::_helpers::rebuilds::full_refresh::configured_full_refresh;
use crate::planner_phases::constants::FULL_REFRESH_DISABLED_REBUILD_CODE;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyBool;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::compiled_views::main::compiled_models::model_name::model_name;
use sqlbuild_python_project::planner_phases::main::planner_error::planner_error;

#[pyfunction]
#[pyo3(signature = (*, model, change_result, full_rebuild_cause))]
pub fn check_full_rebuild_allowed(
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
    let name: String = model_name(model)?;
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
